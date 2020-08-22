use crate::oop::{self, consts as oop_consts, Oop, ValueType};
use crate::runtime::require_class2;
use crate::types::ClassRef;
use crate::types::*;
use crate::util;
use crate::util::PATH_SEP;
use classfile::{
    constant_pool, consts, flags::*, AttributeType, BytesRef, ConstantPool, ConstantPoolType,
    FieldInfo, U2,
};
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

pub fn get_field_ref(cp: &ConstantPool, idx: usize, is_static: bool) -> FieldIdRef {
    let (class_index, name_and_type_index) = constant_pool::get_field_ref(cp, idx);

    //load Field's Class, then init it
    let class = require_class2(class_index, cp).unwrap_or_else(|| {
        panic!(
            "Unknown field class {:?}",
            cp.get(class_index as usize)
                .expect("Missing item")
                .as_cp_item(cp)
        )
    });

    oop::class::init_class(&class);
    oop::class::init_class_fully(&class);

    let (name, desc) = constant_pool::get_name_and_type(cp, name_and_type_index as usize);
    let class = class.get_class();
    class.get_field_id(name, desc, is_static)
}

pub fn build_inited_field_values(class: ClassRef) -> Vec<Oop> {
    let n = {
        let class = class.get_class();
        match &class.kind {
            oop::class::ClassKind::Instance(class_obj) => class_obj.n_inst_fields,
            _ => unreachable!(),
        }
    };

    let null = oop_consts::get_null();
    let mut field_values = vec![null; n];

    let mut cur_cls = class;
    loop {
        let cls = cur_cls.clone();
        let cls = cls.get_class();
        match &cls.kind {
            oop::class::ClassKind::Instance(cls_obj) => {
                cls_obj.inst_fields.iter().for_each(|(_, fir)| {
                    match fir.field.value_type {
                        ValueType::BYTE
                        | ValueType::BOOLEAN
                        | ValueType::CHAR
                        | ValueType::SHORT
                        | ValueType::INT => {
                            field_values[fir.offset] = oop_consts::get_int0();
                        }
                        ValueType::LONG => {
                            field_values[fir.offset] = oop_consts::get_long0();
                        }
                        ValueType::FLOAT => {
                            field_values[fir.offset] = oop_consts::get_float0();
                        }
                        ValueType::DOUBLE => {
                            field_values[fir.offset] = oop_consts::get_double0();
                        }
                        ValueType::OBJECT | ValueType::ARRAY => {
                            //ignore, has been inited by NULL
                        }
                        ValueType::VOID => unreachable!(),
                    }
                });
            }
            _ => unreachable!(),
        }

        if cls.super_class.is_none() {
            break;
        } else {
            cur_cls = cls.super_class.clone().unwrap();
        }
    }

    field_values
}

#[derive(Debug, Clone)]
pub struct FieldId {
    pub offset: usize,
    pub field: Field,
}

#[derive(Clone)]
pub struct Field {
    pub class: ClassRef,
    pub cls_name: BytesRef,
    pub name: BytesRef,
    pub desc: BytesRef,
    pub acc_flags: U2,
    pub value_type: ValueType,
    pub attr_constant_value: Option<Oop>,
}

impl Field {
    pub fn new(cp: &ConstantPool, fi: &FieldInfo, cls_name: BytesRef, class: ClassRef) -> Self {
        let name = constant_pool::get_utf8(cp, fi.name_index as usize).clone();
        let desc = constant_pool::get_utf8(cp, fi.desc_index as usize).clone();
        let value_type = desc.first().unwrap().into();
        let acc_flags = fi.acc_flags;

        let mut attr_constant_value = None;
        for it in fi.attrs.iter() {
            if let AttributeType::ConstantValue {
                constant_value_index,
            } = it
            {
                let v = constant_value(cp, *constant_value_index as usize);
                attr_constant_value = Some(v);
                break;
            }
        }

        Self {
            class,
            cls_name,
            name,
            desc,
            acc_flags,
            value_type,
            attr_constant_value,
        }
    }

    pub fn is_public(&self) -> bool {
        (self.acc_flags & ACC_PUBLIC) == ACC_PUBLIC
    }

    pub fn is_private(&self) -> bool {
        (self.acc_flags & ACC_PRIVATE) == ACC_PRIVATE
    }

    pub fn is_protected(&self) -> bool {
        (self.acc_flags & ACC_PROTECTED) == ACC_PROTECTED
    }

    pub fn is_final(&self) -> bool {
        (self.acc_flags & ACC_FINAL) == ACC_FINAL
    }

    pub fn is_static(&self) -> bool {
        (self.acc_flags & ACC_STATIC) == ACC_STATIC
    }

    pub fn is_volatile(&self) -> bool {
        (self.acc_flags & ACC_VOLATILE) == ACC_VOLATILE
    }

    pub fn get_constant_value(&self) -> Oop {
        match self.value_type {
            ValueType::BYTE
            | ValueType::BOOLEAN
            | ValueType::CHAR
            | ValueType::SHORT
            | ValueType::INT => oop_consts::get_int0(),
            ValueType::LONG => oop_consts::get_long0(),
            ValueType::FLOAT => oop_consts::get_float0(),
            ValueType::DOUBLE => oop_consts::get_double0(),
            ValueType::OBJECT | ValueType::ARRAY => oop_consts::get_null(),
            _ => unreachable!(),
        }
    }

    pub fn get_attr_constant_value(&self) -> Option<Oop> {
        self.attr_constant_value.clone()
    }
}

impl fmt::Debug for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cls_name = unsafe { std::str::from_utf8_unchecked(self.cls_name.as_slice()) };
        let name = unsafe { std::str::from_utf8_unchecked(self.name.as_slice()) };
        let desc = unsafe { std::str::from_utf8_unchecked(self.desc.as_slice()) };
        write!(f, "{}:{}:{}", cls_name, name, desc)
    }
}

fn constant_value(cp: &ConstantPool, v_idx: usize) -> Oop {
    match cp.get(v_idx) {
        Some(ConstantPoolType::Long { v }) => {
            let v = i64::from_be_bytes([v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]]);
            Oop::new_long(v)
        }
        Some(ConstantPoolType::Float { v }) => {
            let v = u32::from_be_bytes([v[0], v[1], v[2], v[3]]);
            let v = f32::from_bits(v);
            Oop::new_float(v)
        }
        Some(ConstantPoolType::Double { v }) => {
            let v = u64::from_be_bytes([v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]]);
            let v = f64::from_bits(v);
            Oop::new_double(v)
        }
        Some(ConstantPoolType::Integer { v }) => {
            let v = i32::from_be_bytes([v[0], v[1], v[2], v[3]]);
            Oop::new_int(v)
        }
        Some(ConstantPoolType::String { string_index }) => {
            let v = constant_pool::get_utf8(cp, *string_index as usize).clone();
            Oop::new_const_utf8(v)
        }
        _ => unreachable!(),
    }
}
