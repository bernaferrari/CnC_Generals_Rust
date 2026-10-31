use ww3d_renderer_3d::Error;
use ww3d_renderer_3d::rendering::wgpu_main_renderer::present_optional_frame;

#[test]
fn temporary_surface_failure_skips_without_finishing_or_claiming_a_frame() {
    for message in [
        "surface occluded",
        "surface timeout",
        "Failed to acquire surface texture: surface occluded",
        "Failed to acquire surface texture: surface timeout",
    ] {
        let mut finished = 0;
        let outcome = present_optional_frame(
            &mut finished,
            |_| Err(Error::RenderError(message.into())),
            |count| {
                *count += 1;
                Ok(())
            },
        );
        assert!(!outcome.unwrap(), "{message}");
        assert_eq!(finished, 0);
    }
}

#[test]
fn available_frame_finishes_and_other_failures_remain_errors() {
    let mut finished = 0;
    assert!(
        present_optional_frame(
            &mut finished,
            |_| Ok(()),
            |count| {
                *count += 1;
                Ok(())
            }
        )
        .unwrap()
    );
    assert_eq!(finished, 1);
    let error = present_optional_frame(
        &mut finished,
        |_| Err(Error::RenderError("surface lost".into())),
        |count| {
            *count += 1;
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("surface lost"));
    assert_eq!(finished, 1);
    let error = present_optional_frame(
        &mut finished,
        |_| Ok(()),
        |count| {
            *count += 1;
            Err(Error::RenderError("finish failed".into()))
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("finish failed"));
    assert_eq!(finished, 2);
}
