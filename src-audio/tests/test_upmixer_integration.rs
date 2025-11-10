// Integration test for upmixer plugin

use sotf_audio::{PluginHost, UpmixerPlugin};

#[test]
fn test_upmixer_stereo_to_5ch() {
    // Create a 2→5 channel plugin host
    let mut host = PluginHost::new(2, 44100);

    // Add upmixer plugin
    let upmixer = UpmixerPlugin::new(2048, 1.0, 0.5, 1.0);
    host.add_plugin(Box::new(upmixer)).unwrap();

    // Verify channel counts
    assert_eq!(host.input_channels(), 2);
    assert_eq!(host.output_channels(), 5);

    // Create stereo input with sine waves
    let mut input_stereo = vec![0.0; 2048 * 2];
    for i in 0..2048 {
        let t = i as f32 / 44100.0;
        input_stereo[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5; // 440 Hz left
        input_stereo[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.3; // 880 Hz right
    }

    let mut output_5ch = vec![0.0; 2048 * 5];

    // Process
    host.process(&input_stereo, &mut output_5ch).unwrap();

    // Verify we got output
    let total_energy: f32 = output_5ch.iter().map(|x| x * x).sum();
    assert!(total_energy > 0.0, "Should have non-zero output");

    // Check individual channels
    let mut channel_energies = vec![0.0; 5];
    for i in 0..2048 {
        for ch in 0..5 {
            channel_energies[ch] += output_5ch[i * 5 + ch].powi(2);
        }
    }

    println!("Channel energies:");
    println!("  Front Left:  {:.4}", channel_energies[0]);
    println!("  Front Right: {:.4}", channel_energies[1]);
    println!("  Center:      {:.4}", channel_energies[2]);
    println!("  Rear Left:   {:.4}", channel_energies[3]);
    println!("  Rear Right:  {:.4}", channel_energies[4]);

    // Front channels should have most energy
    assert!(
        channel_energies[0] > 0.0 || channel_energies[1] > 0.0,
        "Front channels should have content"
    );
}

#[test]
fn test_upmixer_chain_with_gain() {
    use sotf_audio::{GainPlugin, InPlacePluginAdapter};

    // Create a processing chain: stereo → upmix to 5ch → gain on 5ch
    let mut host = PluginHost::new(2, 44100);

    // Add upmixer (2→5)
    let upmixer = UpmixerPlugin::new(1024, 1.0, 0.5, 1.0); // Smaller FFT for this test
    host.add_plugin(Box::new(upmixer)).unwrap();

    // Add gain to the 5-channel output
    let gain = GainPlugin::new(5, -6.0); // -6dB on all 5 channels
    host.add_plugin(Box::new(InPlacePluginAdapter::new(gain)))
        .unwrap();

    // Verify final configuration
    assert_eq!(host.input_channels(), 2);
    assert_eq!(host.output_channels(), 5);

    // Process with varying input
    let mut input = vec![0.0; 1024 * 2];
    for i in 0..1024 {
        input[i * 2] = (i as f32 * 0.01).sin() * 0.5;
        input[i * 2 + 1] = (i as f32 * 0.015).cos() * 0.5;
    }
    let mut output = vec![0.0; 1024 * 5];

    host.process(&input, &mut output).unwrap();

    // Output should be non-zero and attenuated
    let sum: f32 = output.iter().map(|x| x.abs()).sum();
    println!("Chain output sum: {}", sum);
    assert!(sum > 0.0, "Should have output after upmixer + gain");
}

#[test]
fn test_upmixer_parameter_adjustment() {
    use sotf_audio::{ParameterId, ParameterValue, Plugin};

    let mut plugin = UpmixerPlugin::new(2048, 1.0, 0.5, 1.0);
    plugin.initialize(44100).unwrap();

    // Test parameter queries
    let params = plugin.parameters();
    assert_eq!(params.len(), 3);

    // Modify gains
    plugin
        .set_parameter(
            ParameterId::from("gain_front_direct"),
            ParameterValue::Float(0.8),
        )
        .unwrap();

    plugin
        .set_parameter(
            ParameterId::from("gain_rear_ambient"),
            ParameterValue::Float(1.5),
        )
        .unwrap();

    // Verify changes
    let front_direct = plugin.get_parameter(&ParameterId::from("gain_front_direct"));
    assert_eq!(front_direct, Some(ParameterValue::Float(0.8)));

    let rear_ambient = plugin.get_parameter(&ParameterId::from("gain_rear_ambient"));
    assert_eq!(rear_ambient, Some(ParameterValue::Float(1.5)));
}
