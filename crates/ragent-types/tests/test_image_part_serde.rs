use ragent_types::message::{ImageData, MessagePart};

#[test]
fn test_image_part_round_trip() {
    let part = MessagePart::Image(Box::new(ImageData {
        mime_type: "image/png".to_string(),
        path: std::path::PathBuf::from("/tmp/ragent_paste_abc123.png"),
    }));
    let json = serde_json::to_string(&part).expect("serialize image part");
    println!("serialized: {json}");
    let parts = vec![part.clone()];
    let parts_json = serde_json::to_string(&parts).expect("serialize parts");
    println!("serialized parts: {parts_json}");
    let decoded: Vec<MessagePart> = serde_json::from_str(&parts_json).expect("deserialize parts");
    assert_eq!(decoded.len(), 1);
    match &decoded[0] {
        MessagePart::Image(img) => {
            assert_eq!(img.mime_type, "image/png");
            assert_eq!(
                img.path,
                std::path::PathBuf::from("/tmp/ragent_paste_abc123.png")
            );
        }
        _ => panic!("expected Image variant, got {:?}", decoded[0]),
    }
}
