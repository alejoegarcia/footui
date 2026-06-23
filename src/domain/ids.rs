use std::fmt;

macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

id_newtype!(TeamId);
id_newtype!(MatchId);
id_newtype!(StageId);
id_newtype!(GroupId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_display_underlying_source_value() {
        let id = TeamId::from("43922");

        assert_eq!(id.as_str(), "43922");
        assert_eq!(id.to_string(), "43922");
    }
}
