//! Example demonstrating the headphone loss function
//!
//! This example shows how to use the headphone preference score
//! for evaluating frequency response quality.

use autoeq::loss::{headphone_loss, headphone_loss_with_target};
use ndarray::Array1;

fn main() {
    println!("Headphone Loss Function Demo");
    println!("=============================\n");

    // Create frequency grid from 20Hz to 20kHz
    let freq = Array1::logspace(10.0, 1.301, 4.301, 100);
    
    // Example 1: Flat response
    println!("1. Flat Response (0 dB everywhere):");
    let flat_response = Array1::zeros(100);
    let flat_score = headphone_loss(&freq, &flat_response);
    println!("   Score: {:.2}", flat_score);
    println!("   (Lower is better. Penalized for lacking -1 dB/octave slope)\n");
    
    // Example 2: Ideal slope (-1 dB/octave)
    println!("2. Ideal Slope (-1 dB/octave):");
    let ideal_response = freq.mapv(|f: f64| -1.0 * f.log2() + 10.0);
    let ideal_score = headphone_loss(&freq, &ideal_response);
    println!("   Score: {:.2}", ideal_score);
    println!("   (Higher due to RMS in bands, but slope is correct)\n");
    
    // Example 3: Response with bass boost
    println!("3. Bass Boosted Response (+6 dB below 200 Hz):");
    let mut bass_boost = Array1::zeros(100);
    for i in 0..100 {
        if freq[i] < 200.0 {
            bass_boost[i] = 6.0;
        }
    }
    let bass_score = headphone_loss(&freq, &bass_boost);
    println!("   Score: {:.2}", bass_score);
    println!("   (Penalized for excessive bass)\n");
    
    // Example 4: Response with 1kHz peak
    println!("4. Response with 8 dB peak at 1 kHz:");
    let mut peak_response = Array1::zeros(100);
    for i in 0..100 {
        if freq[i] > 800.0 && freq[i] < 1200.0 {
            peak_response[i] = 8.0;
        }
    }
    let peak_score = headphone_loss(&freq, &peak_response);
    println!("   Score: {:.2}", peak_score);
    println!("   (Heavily penalized for midrange peak)\n");
    
    // Example 5: Using with target curve
    println!("5. Measured vs Target Curve:");
    let measured = Array1::from_elem(100, 3.0); // Constant 3 dB
    let target = Array1::from_elem(100, 3.0);   // Same target
    let target_score = headphone_loss_with_target(&freq, &measured, &target);
    println!("   Score when matching target: {:.2}", target_score);
    
    // Offset from target
    let measured_offset = Array1::from_elem(100, 5.0); // 2 dB higher
    let offset_score = headphone_loss_with_target(&freq, &measured_offset, &target);
    println!("   Score with 2 dB offset: {:.2}", offset_score);
    
    println!("\nSummary:");
    println!("--------");
    println!("The headphone loss function evaluates preference based on:");
    println!("• Slope deviation from -1 dB/octave target");
    println!("• RMS deviation in frequency bands");
    println!("• Peak-to-peak variations");
    println!("• Frequency-weighted importance (bass/midrange > treble)");
}