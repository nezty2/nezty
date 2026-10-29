//! JSON-instellingen van andere apps (Discord, browsers): één waarde lezen of zetten via een JSON-pointer,
//! zonder de rest van het bestand te veranderen.

use crate::error::{AppError, AppResult};
use serde_json::{Map, Value};
use std::path::Path;

pub fn read(path: &Path) -> AppResult<Value> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| AppError::msg(format!("{}: {e}", path.display())))
}

pub fn get(path: &Path, pointer: &str) -> AppResult<Option<Value>> {
    Ok(read(path)?.pointer(pointer).cloned())
}

fn parts(pointer: &str) -> Vec<String> {
    pointer.trim_start_matches('/').split('/').map(|p| p.replace("~1", "/").replace("~0", "~")).collect()
}

/// Zet de waarde op `pointer`, of haalt hem weg bij `None`. Ontbrekende tussenobjecten worden aangemaakt.
pub fn set_pointer(path: &Path, pointer: &str, value: Option<Value>, pretty: bool) -> AppResult<()> {
    let mut root = read(path)?;
    let keys = parts(pointer);
    let (last, parents) = keys.split_last().ok_or_else(|| AppError::msg("empty JSON pointer"))?;
    let mut cur = &mut root;
    for key in parents {
        let obj = cur.as_object_mut().ok_or_else(|| AppError::msg("JSON value is not an object"))?;
        if !obj.get(key).is_some_and(Value::is_object) {
            if value.is_none() {
                return Ok(());
            }
            obj.insert(key.clone(), Value::Object(Map::new()));
        }
        cur = obj.get_mut(key).ok_or_else(|| AppError::msg("JSON pointer"))?;
    }
    let obj = cur.as_object_mut().ok_or_else(|| AppError::msg("JSON value is not an object"))?;
    match value {
        Some(v) => {
            obj.insert(last.clone(), v);
        }
        None => {
            obj.remove(last);
            // Lege tussenobjecten die alleen voor deze waarde bestonden ook weer weghalen.
            for depth in (0..parents.len()).rev() {
                let child = format!("/{}", parents[..=depth].iter().map(|p| p.replace('~', "~0").replace('/', "~1")).collect::<Vec<_>>().join("/"));
                if !root.pointer(&child).and_then(Value::as_object).is_some_and(Map::is_empty) {
                    break;
                }
                let parent = format!("/{}", parents[..depth].iter().map(|p| p.replace('~', "~0").replace('/', "~1")).collect::<Vec<_>>().join("/"));
                let holder = if depth == 0 { root.as_object_mut() } else { root.pointer_mut(&parent).and_then(Value::as_object_mut) };
                if let Some(h) = holder {
                    h.remove(&parents[depth]);
                }
            }
        }
    }
    let text = if pretty { serde_json::to_string_pretty(&root) } else { serde_json::to_string(&root) }.map_err(|e| AppError::msg(e.to_string()))?;
    std::fs::write(path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_creates_and_removes() {
        let dir = std::env::temp_dir().join(format!("nezty-json-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.json");
        std::fs::write(&f, r#"{"keep":1}"#).unwrap();
        set_pointer(&f, "/hardware_acceleration_mode/enabled", Some(Value::Bool(false)), false).unwrap();
        assert_eq!(get(&f, "/hardware_acceleration_mode/enabled").unwrap(), Some(Value::Bool(false)));
        assert_eq!(get(&f, "/keep").unwrap(), Some(Value::from(1)));
        set_pointer(&f, "/hardware_acceleration_mode/enabled", None, false).unwrap();
        assert_eq!(get(&f, "/hardware_acceleration_mode/enabled").unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
