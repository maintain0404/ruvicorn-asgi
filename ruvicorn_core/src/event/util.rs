use crate::errors::AsgiSpecError;
use pyo3::{types::PyDict, FromPyObject};

pub fn get_item_with_casting<'t, T>(dict: &'t PyDict, key: &str) -> Result<T, AsgiSpecError>
where
    T: FromPyObject<'t>,
{
    if let Some(pitem) = dict.get_item(key) {
        match pitem.extract::<T>() {
            Ok(ritem) => Ok(ritem),
            Err(_) => Err(AsgiSpecError {}),
        }
    } else {
        Err(AsgiSpecError {})
    }
}

pub fn get_item_with_default<'t, T: FromPyObject<'t>>(
    dict: &'t PyDict,
    key: &str,
    default: T,
) -> T {
    if let Some(pitem) = dict.get_item(key) {
        if let Ok(ritem) = pitem.extract::<'t, T>() {
            return ritem;
        } else {
            return default;
        }
    } else {
        return default;
    }
}
