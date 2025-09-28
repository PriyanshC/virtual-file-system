use vfs::filesys::{Filesys, BufferCacheStrategy};

static mut FILESYS: Filesys = Filesys::init();

const PATH: &str = "./virt.disk";
const DISK_BLOCKS: u64 = 30;
const SAMPLE_DATA: &[u8] = b"Cake or pie? I can tell a lot about you by which one you pick. It may seem silly, but cake people and pie people are really different. I know which one I hope you are, but that's not for me to decide. So, what is it? Cake or pie?";
const FILE_OFFSET: i64 = 0;

fn main() {

  unsafe {
    /* Initialise a new disk. Alternatively, load an existing one  */
    let _ = std::fs::remove_file(PATH);
    // FILESYS.new_disk(PATH, DISK_BLOCKS, BufferCacheStrategy::None);
    FILESYS.new_disk(PATH, DISK_BLOCKS, BufferCacheStrategy::Arc { capacity: 8 });
    FILESYS.init_free_map();
    
    /* File should not already exist */
    assert!(FILESYS.open_file("a.txt").is_none());

    /* Create a file to store our data */
    let success = FILESYS.create_file("a.txt", SAMPLE_DATA.len() as u64);
    assert!(success);

    /* We should see the file listed */
    let files = FILESYS.list("/").expect("directory exists");
    assert!(files.contains(&String::from("a.txt")));

    /* Open a handle to the file and write contents */
    let mut file = FILESYS.open_file("a.txt").expect("couldn't open file");

    let bytes_written = FILESYS.file_write(&mut file, SAMPLE_DATA, FILE_OFFSET);
    assert_eq!(bytes_written, SAMPLE_DATA.len() as i64);

    /* Read from where we have written into a new buffer. Repeat multiple times to test cache */
    file.seek_start();
    let mut buf = [u8::MAX; SAMPLE_DATA.len()];
    let bytes_read = FILESYS.file_read(&mut file, &mut buf, FILE_OFFSET);
    assert_eq!(bytes_read, SAMPLE_DATA.len() as i64);

    file.seek_start();
    let mut buf = [u8::MAX; SAMPLE_DATA.len()];
    let bytes_read = FILESYS.file_read(&mut file, &mut buf, FILE_OFFSET);
    assert_eq!(bytes_read, SAMPLE_DATA.len() as i64);

    file.seek_start();
    let mut buf = [u8::MAX; SAMPLE_DATA.len()];
    let bytes_read = FILESYS.file_read(&mut file, &mut buf, FILE_OFFSET);
    assert_eq!(bytes_read, SAMPLE_DATA.len() as i64);

    /* Confirm and display our previously written contents */
    println!(
      "{:?}\n",
      String::from_utf8(buf.to_vec()).expect("corruped data")
    );
    assert_eq!(SAMPLE_DATA, buf);

    /* Display number of read and write calls to DISK */
    FILESYS.display_disk_stats();
  }
}
