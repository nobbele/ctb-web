use macroquad::prelude::*;
use serde::ser::SerializeMap;
use std::intrinsics::transmute;

/// Sets a global configuration value. Uses local sotrage on web and "data/config.json" on native.
pub fn set_value<T: serde::Serialize>(key: &str, value: T) {
    let value = serde_json::to_value(value).unwrap();
    #[cfg(target_family = "wasm")]
    {
        let storage = web_sys::window().unwrap().local_storage().unwrap().unwrap();
        storage.set_item(key, &value.to_string()).unwrap();
    }
    #[cfg(not(target_family = "wasm"))]
    {
        use std::collections::HashMap;
        let mut config: HashMap<String, serde_json::Value> = match std::fs::OpenOptions::new()
            .read(true)
            .open("data/config.json")
        {
            Ok(file) => serde_json::from_reader(file).unwrap(),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    HashMap::new()
                } else {
                    panic!("{:?}", e)
                }
            }
        };

        config.insert(key.to_string(), value);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open("data/config.json")
            .unwrap();
        serde_json::to_writer(file, &config).unwrap();
    }
}

/// Gets a global configuration value. Uses local sotrage on web and "data/config.json" on native.
pub fn get_value<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
    #[cfg(target_family = "wasm")]
    {
        let storage = web_sys::window().unwrap().local_storage().unwrap().unwrap();
        let value = storage.get_item(key).unwrap()?;
        serde_json::from_str(&value).unwrap()
    }
    #[cfg(not(target_family = "wasm"))]
    {
        let document = match std::fs::OpenOptions::new()
            .read(true)
            .open("data/config.json")
        {
            Ok(file) => serde_json::from_reader(file).unwrap(),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    serde_json::Value::Object(serde_json::Map::new())
                } else {
                    panic!("{:?}", e)
                }
            }
        };
        serde_json::from_value(document.get(key)?.clone()).ok()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct KeyBinds {
    pub right: KeyCode,
    pub left: KeyCode,
    pub dash: KeyCode,
}

impl serde::Serialize for KeyBinds {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("left", &(self.left as u32))?;
        map.serialize_entry("right", &(self.right as u32))?;
        map.serialize_entry("dash", &(self.dash as u32))?;
        map.end()
    }
}

/// [`macroquad::KeyCode`] doesn't implement [`serde::Deserialize`] or [`serde::Serialize`]..
struct KeyBindVisitor;

impl<'de> serde::de::Visitor<'de> for KeyBindVisitor {
    type Value = KeyBinds;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("Expected a map with the keys 'left', 'right', and 'dash'")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut binds = KeyBinds {
            right: KeyCode::D,
            left: KeyCode::A,
            dash: KeyCode::RightShift,
        };
        while let Some((key, value)) = map.next_entry::<_, u32>()? {
            let value = unsafe { transmute(value) };
            match key {
                "left" => {
                    binds.left = value;
                }
                "right" => {
                    binds.right = value;
                }
                "dash" => {
                    binds.dash = value;
                }
                _ => panic!(),
            }
        }
        Ok(binds)
    }
}

impl<'de> serde::Deserialize<'de> for KeyBinds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(KeyBindVisitor)
    }
}
