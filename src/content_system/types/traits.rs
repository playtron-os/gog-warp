pub trait EntryUtils {
    fn path(&self) -> String;
    fn compressed_size(&self) -> i64;
    fn size(&self) -> i64;
    fn is_support(&self) -> bool;
    fn is_dir(&self) -> bool;
}
