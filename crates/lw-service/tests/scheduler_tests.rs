use lw_service::scheduler::{select_next_wallpaper, get_wallpaper_files};
use std::fs::File;
use std::path::{Path, PathBuf};

#[test]
fn test_get_wallpaper_files_filtering() {
    let temp_dir = std::env::temp_dir().join("liem_wallpaper_scheduler_test");
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create valid wallpaper files
    File::create(temp_dir.join("a.png")).unwrap();
    File::create(temp_dir.join("c.jpg")).unwrap();
    File::create(temp_dir.join("b.bmp")).unwrap();
    File::create(temp_dir.join("d.jpeg")).unwrap();

    // Create invalid files/directories to ensure they are filtered out
    File::create(temp_dir.join("readme.txt")).unwrap();
    File::create(temp_dir.join("config.toml")).unwrap();
    let _ = std::fs::create_dir(temp_dir.join("subfolder.png"));

    let files = get_wallpaper_files(&temp_dir);

    // Clean up
    let _ = std::fs::remove_file(temp_dir.join("a.png"));
    let _ = std::fs::remove_file(temp_dir.join("b.bmp"));
    let _ = std::fs::remove_file(temp_dir.join("c.jpg"));
    let _ = std::fs::remove_file(temp_dir.join("d.jpeg"));
    let _ = std::fs::remove_file(temp_dir.join("readme.txt"));
    let _ = std::fs::remove_file(temp_dir.join("config.toml"));
    let _ = std::fs::remove_dir(temp_dir.join("subfolder.png"));
    let _ = std::fs::remove_dir(&temp_dir);

    // Verify correct sorting (a, b, c, d) and extension matching
    assert_eq!(files.len(), 4);
    assert_eq!(files[0].file_name().unwrap(), "a.png");
    assert_eq!(files[1].file_name().unwrap(), "b.bmp");
    assert_eq!(files[2].file_name().unwrap(), "c.jpg");
    assert_eq!(files[3].file_name().unwrap(), "d.jpeg");
}

#[test]
fn test_select_next_wallpaper_sequential() {
    let wallpapers = vec![
        PathBuf::from("wp1.png"),
        PathBuf::from("wp2.png"),
        PathBuf::from("wp3.png"),
    ];

    // Starts at first when current is not in list or empty
    let next = select_next_wallpaper(&wallpapers, Path::new(""), false).unwrap();
    assert_eq!(next, PathBuf::from("wp1.png"));

    // Moves sequentially
    let next = select_next_wallpaper(&wallpapers, Path::new("wp1.png"), false).unwrap();
    assert_eq!(next, PathBuf::from("wp2.png"));

    let next = select_next_wallpaper(&wallpapers, Path::new("wp2.png"), false).unwrap();
    assert_eq!(next, PathBuf::from("wp3.png"));

    // Wraps around
    let next = select_next_wallpaper(&wallpapers, Path::new("wp3.png"), false).unwrap();
    assert_eq!(next, PathBuf::from("wp1.png"));
}

#[test]
fn test_select_next_wallpaper_shuffle() {
    let wallpapers = vec![
        PathBuf::from("wp1.png"),
        PathBuf::from("wp2.png"),
        PathBuf::from("wp3.png"),
    ];

    // Shuffle should avoid selecting the current wallpaper if possible
    let mut different_count = 0;
    for _ in 0..50 {
        let next = select_next_wallpaper(&wallpapers, Path::new("wp1.png"), true).unwrap();
        assert!(wallpapers.contains(&next));
        if next.as_path() != Path::new("wp1.png") {
            different_count += 1;
        }
    }
    // High probability that it chooses a different one almost every time
    assert!(different_count > 40);
}
