/// Values the spec marks as "preliminary, leave room for change".
/// Every rule reads them from here; nothing else hard-codes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    pub name_max_len: usize,
    pub port_rows_per_u: u32,
    pub port_cols: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            name_max_len: 10,
            port_rows_per_u: 1,
            port_cols: 5,
        }
    }
}
