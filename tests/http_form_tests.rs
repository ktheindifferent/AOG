use rouille::{post_input, Request};

fn multipart_request(body: &str) -> Request {
    Request::fake_http(
        "POST",
        "/api/command",
        vec![(
            "Content-Type".to_owned(),
            "multipart/form-data; boundary=aog-test".to_owned(),
        )],
        body.as_bytes().to_vec(),
    )
}

#[test]
fn command_form_parses_multipart_field() {
    let request = multipart_request(concat!(
        "--aog-test\r\n",
        "Content-Disposition: form-data; name=\"input_command\"\r\n\r\n",
        "pump status\r\n",
        "--aog-test--\r\n",
    ));

    let input = post_input!(&request, { input_command: String }).unwrap();
    assert_eq!(input.input_command, "pump status");
}

#[test]
fn command_form_rejects_malformed_multipart_field() {
    let request = multipart_request(concat!(
        "--aog-test\r\n",
        "Content-Disposition: form-data\r\n\r\n",
        "pump status\r\n",
        "--aog-test--\r\n",
    ));

    // Missing field metadata must remain an error after the dependency patch.
    assert!(post_input!(&request, { input_command: String }).is_err());
}
