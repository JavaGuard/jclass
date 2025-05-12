use crate::classfile_constants::{JVM_CONSTANT_Class, JVM_CONSTANT_Double, JVM_CONSTANT_Dynamic, JVM_CONSTANT_Fieldref, JVM_CONSTANT_Float, JVM_CONSTANT_Integer, JVM_CONSTANT_InterfaceMethodref, JVM_CONSTANT_InvokeDynamic, JVM_CONSTANT_Long, JVM_CONSTANT_MethodHandle, JVM_CONSTANT_MethodType, JVM_CONSTANT_Methodref, JVM_CONSTANT_Module, JVM_CONSTANT_NameAndType, JVM_CONSTANT_Package, JVM_CONSTANT_String, JVM_CONSTANT_Utf8, _bindgen_ty_3};
use crate::common::error::{MessageError, Result};

#[repr(C, align(8))]
#[derive(Debug)]
pub struct DataRange {
    pub start: usize,
    pub end: usize,
}

/// consts 中每个元素为每个常量的截止索引（不含索引指向的值），首元素为第一个常量的开始索引
#[repr(C, align(8))]
#[derive(Debug)]
pub struct SimpleClassInfo {
    pub consts: Vec<usize>,
    pub fields_start: usize,
    pub methods_start: usize,
    pub method_codes: Vec<(usize, usize)>,
    pub attributes_start: usize,
    pub specify_attribute: Option<DataRange>,
}

const CODE_ATTR_NAME: &[u8] = "Code".as_bytes();

const CODE_ATTR_NAME_LEN: usize = CODE_ATTR_NAME.len();

#[inline]
pub fn fast_scan_class(data: & [u8], attribute_name: &[u8], not_check_attr: bool) -> Result<Option<SimpleClassInfo>> {
    // magic + minor_version + major_version
    let mut index = 8;
    let constant_size = get_u16_from_data(data, &mut index)?;
    let constant_size = constant_size as usize;
    let mut data_key_index = 0;
    let mut name_found = not_check_attr;
    let mut consts = Vec::with_capacity(constant_size);
    unsafe {
        consts.set_len(constant_size);
    }
    consts[0] = index;
    let attribute_name_len = attribute_name.len();
    let mut find_code = true;
    let mut code_index = 0;
    for i in 1..constant_size {
        match get_constant_value_size(data, &mut index, attribute_name, attribute_name_len, name_found, find_code)? {
            1 => {
                name_found = true;
                data_key_index = i;
            }
            2 => {
                find_code = false;
                code_index = i;
            }
            _ => {}
        }
        consts[i] = index;
    }
    if name_found {
        let constants_end = index;
        // access_flags + class_index + superclass_index
        index += 6;
        // interface
        let interface_size = get_u16_from_data(data, &mut index)?;
        index += (interface_size as usize) << 1;
        // field
        let fields_start = index;
        handle_field_or_method(data, &mut index)?;
        // method
        let methods_start = index;
        // handle_field_or_method(data, &mut index)?;
        let code_index_bytes = (code_index as u16).to_be_bytes();
        let size = get_u16_from_data(data, &mut index)?;
        let size = size as usize;
        let mut method_codes = Vec::with_capacity(size);
        unsafe {
            method_codes.set_len(size);
        }
        for i in 0..size {
            // access_flags + name + descriptor
            index += 6;
            let attr_size = get_u16_from_data(data, &mut index)?;
            let mut code_range = (0, 0);
            for _ in 0..attr_size {
                // name
                let start = index;
                index += 2;
                let data_size = get_u32_from_data(data, &mut index)?;
                index += data_size as usize;
                if &data[start..start+2] == &code_index_bytes {
                    code_range = (start, index);
                }
            }
            method_codes[i] = code_range;
        }

        // attribute
        let attributes_start = index;
        let attr_size = get_u16_from_data(data, &mut index)?;
        let mut specify_attribute = None;
        let data_key_index = data_key_index as u16;
        for _ in 0..attr_size {
            // name
            let name_index = get_u16_from_data(data, &mut index)?;
            let data_size = get_u32_from_data(data, &mut index)?;
            let start = index;
            index += data_size as usize;
            if name_index == data_key_index {
                return if index > data.len() {
                    Err(MessageError::new("读取命中的属性内容时越界"))
                } else {
                    specify_attribute = Some(DataRange {
                        start,
                        end: index,
                    });
                    break;
                }
            }
        }
        Ok(Some(SimpleClassInfo {
            consts,
            fields_start,
            methods_start,
            method_codes,
            attributes_start,
            specify_attribute,
        }))
    } else {
        Ok(None)
    }
}

#[inline(always)]
fn handle_attributes(data: &[u8], index: &mut usize) -> Result<()> {
    let attr_size = get_u16_from_data(data, index)?;
    for _ in 0..attr_size {
        // name
        *index += 2;
        let data_size = get_u32_from_data(data, index)?;
        *index += data_size as usize;
    }
    Ok(())
}

#[inline(always)]
pub fn handle_field_or_method(data: &[u8], index: &mut usize) -> Result<()> {
    let size = get_u16_from_data(data, index)?;
    for _ in 0..size {
        // access_flags + name + descriptor
        *index += 6;
        handle_attributes(data, index)?;
    }
    Ok(())
}

#[inline]
fn get_constant_value_size(data: &[u8], index: &mut usize, attribute_name: &[u8], attribute_name_len: usize, name_found: bool, find_code: bool) -> Result<i8> {
    let type_ = match data.get(*index) {
        None => {
            return Err(MessageError::new("读取常量类型时越界"));
        }
        Some(v) => *v
    };
    *index += 1;
    *index += match type_ as _bindgen_ty_3 {
        JVM_CONSTANT_Utf8 => {
            let str_size = get_u16_from_data(data, index)?;
            let str_size = str_size as usize;
            if find_code && str_size == CODE_ATTR_NAME_LEN {
                let end = *index + CODE_ATTR_NAME_LEN;
                if end > data.len() {
                    return Err(MessageError::new("读取utf8越界"))
                }

                if &data[*index..end] == CODE_ATTR_NAME {
                    *index += CODE_ATTR_NAME_LEN;
                    return Ok(2);
                }
            }
            if name_found || str_size != attribute_name_len {
                *index += str_size;
                return Ok(0);
            } else {
                let start = *index;
                *index += str_size;
                if *index > data.len() {
                    return Err(MessageError::new("读取utf8越界"))
                }

                let eq = &data[start..*index] == attribute_name;
                return Ok(eq as i8);
            }
        }
        JVM_CONSTANT_Integer | JVM_CONSTANT_Float => {
            size_of::<i32>()
        }
        JVM_CONSTANT_Long | JVM_CONSTANT_Double=> {
            size_of::<i64>()
        }
        JVM_CONSTANT_Class |
        JVM_CONSTANT_String | JVM_CONSTANT_Module |
        JVM_CONSTANT_Package | JVM_CONSTANT_MethodType => {
            size_of::<u16>()
        }
        JVM_CONSTANT_Fieldref | JVM_CONSTANT_Methodref |
        JVM_CONSTANT_InterfaceMethodref | JVM_CONSTANT_NameAndType |
        JVM_CONSTANT_Dynamic | JVM_CONSTANT_InvokeDynamic => {
            size_of::<[u16;2]>()
        }
        JVM_CONSTANT_MethodHandle => {
            size_of::<u16>() + size_of::<u8>()
        }
        _ => {
            0
        }
    };
    Ok(0)
}

#[inline(always)]
pub fn get_u16_from_data(data: &[u8], index: &mut usize) -> Result<u16> {
    let start = *index;
    *index += 2;
    if *index > data.len() {
        return Err(MessageError::new("读取u16越界"))
    }
    unsafe {
        let ptr = data.as_ptr().add(start) as *const u16;
        Ok(u16::from_be(ptr.read_unaligned()))
    }
}

#[inline(always)]
pub fn get_u32_from_data(data: &[u8], index: &mut usize) -> Result<u32> {
    let start = *index;
    *index += 4;
    if *index > data.len() {
        return Err(MessageError::new("读取u32越界"))
    }
    unsafe {
        let ptr = data.as_ptr().add(start) as *const u32;
        Ok(u32::from_be(ptr.read_unaligned()))
    }
}