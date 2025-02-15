use uuid::{Uuid};
use once_cell::sync::{OnceCell, Lazy};

use std::ops::{Deref};
use std::collections::*;
use std::sync::*;

#[cfg(feature="serde_support")] use serde::*;
#[cfg(feature="serde_support")] use serde::de::*;

static IDS_FOR_NAMES: Lazy<RwLock<HashMap<String, SubProgramNameId>>>   = Lazy::new(|| RwLock::new(HashMap::new()));
static NAMES_FOR_IDS: Lazy<RwLock<Vec<String>>>                         = Lazy::new(|| RwLock::new(vec![]));

fn id_for_name(name: &str) -> SubProgramNameId {
    let id = { IDS_FOR_NAMES.read().unwrap().get(name).copied() };

    if let Some(id) = id {
        // ID already exists
        id
    } else {
        // Create a new ID
        let id = {
            let mut names_for_ids   = NAMES_FOR_IDS.write().unwrap();
            let id                  = names_for_ids.len();
            names_for_ids.push(name.into());

            id
        };
        let id = SubProgramNameId(id);

        // Store the mapping
        let mut ids_for_names = IDS_FOR_NAMES.write().unwrap();
        ids_for_names.insert(name.into(), id);

        id
    }
}

fn name_for_id(id: SubProgramNameId) -> Option<String> {
    (*NAMES_FOR_IDS).read().unwrap().get(id.0).cloned()
}

///
/// A unique identifier for a subprogram in a scene
///
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct SubProgramId(SubProgramIdValue);

///
/// A subprogram name ID
///
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
struct SubProgramNameId(usize);

#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
enum SubProgramIdValue {
    /// A subprogram identified with a well-known name
    Named(SubProgramNameId),

    /// A subprogram identified with a GUID
    Guid(Uuid),

    /// A task created by a named subprogram. The second 'usize' value is a unique serial number for this task
    ///
    /// Tasks differ from subprograms in that they have a limited lifespan and read an input stream specified at creation
    NamedTask(SubProgramNameId, usize),

    /// A task created by a GUID subprogram. The 'usize' value is a unique serial number for this task
    ///
    /// Tasks differ from subprograms in that they have a limited lifespan and read an input stream specified at creation
    GuidTask(Uuid, usize),
}

///
/// A static subprogram ID can be used to declare a subprogram ID in a static variable
///
pub struct StaticSubProgramId(&'static str, OnceCell<SubProgramId>);

impl SubProgramId {
    ///
    /// Creates a new unique subprogram id
    ///
    #[inline]
    #[allow(clippy::new_without_default)]   // As this isn't a default value, it's a *new* value, there's no default subprogram ID
    pub fn new() -> SubProgramId {
        SubProgramId(SubProgramIdValue::Guid(Uuid::new_v4()))
    }

    ///
    /// Creates a subprogram ID with a well-known name
    ///
    #[inline]
    pub fn called(name: &str) -> SubProgramId {
        SubProgramId(SubProgramIdValue::Named(id_for_name(name)))
    }

    ///
    /// Creates a command subprogram ID (with a particular sequence number)
    ///
    pub (crate) fn with_command_id(&self, command_sequence_number: usize) -> SubProgramId {
        match self.0 {
            SubProgramIdValue::Named(name_num)          |
            SubProgramIdValue::NamedTask(name_num, _)   => SubProgramId(SubProgramIdValue::NamedTask(name_num, command_sequence_number)),

            SubProgramIdValue::Guid(guid)               |
            SubProgramIdValue::GuidTask(guid, _)        => SubProgramId(SubProgramIdValue::GuidTask(guid, command_sequence_number)),
        }
    }
}

impl StaticSubProgramId {
    ///
    /// Creates a subprogram ID with a well-known name
    ///
    #[inline]
    pub const fn called(name: &'static str) -> StaticSubProgramId {
        StaticSubProgramId(name, OnceCell::new())
    }
}

impl Deref for StaticSubProgramId {
    type Target = SubProgramId;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.1.get()
            .unwrap_or_else(|| {
                let subprogram = SubProgramId::called(self.0);
                self.1.set(subprogram).ok();
                self.1.get().unwrap()
            })
    }
}

#[cfg(feature="serde_support")]
impl Serialize for SubProgramNameId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer 
    {
        let name_string = name_for_id(*self).unwrap();
        serializer.serialize_str(&name_string)
    }
}

#[cfg(feature="serde_support")]
impl<'de> Deserialize<'de> for SubProgramNameId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de> 
    {
        struct StrVisitor;
        impl<'de> Visitor<'de> for StrVisitor {
            type Value = String;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("A string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(value.to_string())
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(value)
            }
        }

        let name_string = deserializer.deserialize_str(StrVisitor)?;
        Ok(id_for_name(&name_string))
    }
}

#[cfg(all(test, feature="json"))]
mod test {
    use super::*;

    use serde_json::json;

    #[test]
    pub fn serialize_name() {
        let subprogram_id   = id_for_name("test");
        let json_name       = subprogram_id.serialize(serde_json::value::Serializer).unwrap();

        assert!(json_name == json!["test"]);
    }

    #[test]
    pub fn deserialize_name() {
        let deserialized_name = SubProgramNameId::deserialize(json!["another_test"]).unwrap();

        assert!(deserialized_name == id_for_name("another_test"));
    }
}
