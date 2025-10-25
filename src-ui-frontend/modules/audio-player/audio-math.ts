/**
 * Audio mathematics utility functions
 * Handles complex pressure calculations and signal operations
 */

export interface ComplexNumber {
  real: number;
  imag: number;
}

export interface FrequencyResponse {
  frequencies: number[];
  magnitudes: number[]; // in dB
  phases: number[]; // in degrees
}

/**
 * Convert magnitude (dB) and phase (degrees) to complex pressure
 * Complex pressure = 10^(dB/20) * e^(j*phase)
 */
export function magnitudePhaseToComplex(
  magnitudeDB: number,
  phaseDeg: number,
): ComplexNumber {
  // Convert dB to linear magnitude
  const magnitude = Math.pow(10, magnitudeDB / 20);

  // Convert phase from degrees to radians
  const phaseRad = (phaseDeg * Math.PI) / 180;

  // Complex number in rectangular form: magnitude * (cos(phase) + j*sin(phase))
  return {
    real: magnitude * Math.cos(phaseRad),
    imag: magnitude * Math.sin(phaseRad),
  };
}

/**
 * Convert complex pressure to magnitude (dB) and phase (degrees)
 */
export function complexToMagnitudePhase(complex: ComplexNumber): {
  magnitudeDB: number;
  phaseDeg: number;
} {
  // Calculate magnitude
  const magnitude = Math.sqrt(
    complex.real * complex.real + complex.imag * complex.imag,
  );
  const magnitudeDB = 20 * Math.log10(Math.max(magnitude, 1e-10)); // Avoid log(0)

  // Calculate phase in radians
  const phaseRad = Math.atan2(complex.imag, complex.real);

  // Convert to degrees
  const phaseDeg = (phaseRad * 180) / Math.PI;

  return { magnitudeDB, phaseDeg };
}

/**
 * Add two complex numbers
 */
export function complexAdd(a: ComplexNumber, b: ComplexNumber): ComplexNumber {
  return {
    real: a.real + b.real,
    imag: a.imag + b.imag,
  };
}

/**
 * Sum multiple frequency responses using complex addition
 * This performs pressure addition, not SPL addition
 *
 * @param responses Array of frequency responses to sum
 * @returns Combined frequency response with complex-summed magnitudes and phases
 */
export function sumFrequencyResponses(
  responses: FrequencyResponse[],
): FrequencyResponse | null {
  if (responses.length === 0) {
    return null;
  }

  if (responses.length === 1) {
    return {
      frequencies: [...responses[0].frequencies],
      magnitudes: [...responses[0].magnitudes],
      phases: [...responses[0].phases],
    };
  }

  // Verify all responses have the same frequency points
  const refFreqs = responses[0].frequencies;
  const numPoints = refFreqs.length;

  for (let i = 1; i < responses.length; i++) {
    if (responses[i].frequencies.length !== numPoints) {
      console.error(
        "Cannot sum responses with different frequency point counts",
      );
      return null;
    }

    // Check if frequencies match (with tolerance)
    for (let j = 0; j < numPoints; j++) {
      if (Math.abs(responses[i].frequencies[j] - refFreqs[j]) > 0.1) {
        console.error("Cannot sum responses with different frequency points");
        return null;
      }
    }
  }

  // Perform complex addition at each frequency point
  const resultMagnitudes: number[] = [];
  const resultPhases: number[] = [];

  for (let freqIdx = 0; freqIdx < numPoints; freqIdx++) {
    // Convert each response to complex form
    const complexValues: ComplexNumber[] = responses.map((response) =>
      magnitudePhaseToComplex(
        response.magnitudes[freqIdx],
        response.phases[freqIdx],
      ),
    );

    // Sum all complex values
    let sum: ComplexNumber = { real: 0, imag: 0 };
    for (const complex of complexValues) {
      sum = complexAdd(sum, complex);
    }

    // Convert back to magnitude and phase
    const { magnitudeDB, phaseDeg } = complexToMagnitudePhase(sum);
    resultMagnitudes.push(magnitudeDB);
    resultPhases.push(phaseDeg);
  }

  return {
    frequencies: [...refFreqs],
    magnitudes: resultMagnitudes,
    phases: resultPhases,
  };
}

/**
 * Sum left and right channel responses using complex addition
 * Special case of sumFrequencyResponses for stereo pairs
 */
export function sumLeftRightChannels(
  leftResponse: FrequencyResponse,
  rightResponse: FrequencyResponse,
): FrequencyResponse | null {
  return sumFrequencyResponses([leftResponse, rightResponse]);
}

/**
 * Average left and right channel responses using complex averaging
 * This divides the complex sum by 2 to get the average
 */
export function averageLeftRightChannels(
  leftResponse: FrequencyResponse,
  rightResponse: FrequencyResponse,
): FrequencyResponse | null {
  // First get the sum
  const sumResult = sumFrequencyResponses([leftResponse, rightResponse]);

  if (!sumResult) {
    return null;
  }

  // Convert to complex, divide by 2, convert back
  const averagedMagnitudes: number[] = [];
  const averagedPhases: number[] = [];

  for (let i = 0; i < sumResult.frequencies.length; i++) {
    // Convert to complex
    const complex = magnitudePhaseToComplex(
      sumResult.magnitudes[i],
      sumResult.phases[i],
    );

    // Divide by 2 (average)
    const avgComplex: ComplexNumber = {
      real: complex.real / 2,
      imag: complex.imag / 2,
    };

    // Convert back to magnitude/phase
    const { magnitudeDB, phaseDeg } = complexToMagnitudePhase(avgComplex);
    averagedMagnitudes.push(magnitudeDB);
    averagedPhases.push(phaseDeg);
  }

  return {
    frequencies: [...sumResult.frequencies],
    magnitudes: averagedMagnitudes,
    phases: averagedPhases,
  };
}

/**
 * Normalize phase to [-180, 180] range
 */
export function normalizePhase(phaseDeg: number): number {
  let normalized = phaseDeg;

  // Wrap to [-180, 180]
  while (normalized > 180) {
    normalized -= 360;
  }
  while (normalized < -180) {
    normalized += 360;
  }

  return normalized;
}

/**
 * Unwrap phase to remove discontinuities
 * Converts phase to continuous form by adding/subtracting 360° where jumps occur
 */
export function unwrapPhase(phases: number[]): number[] {
  if (phases.length === 0) return [];

  const unwrapped = [phases[0]];

  for (let i = 1; i < phases.length; i++) {
    let diff = phases[i] - phases[i - 1];

    // If jump is > 180°, it's likely a wrap
    if (diff > 180) {
      // Wrapped down, add 360 to continue
      unwrapped.push(unwrapped[i - 1] + (phases[i] - phases[i - 1] + 360));
    } else if (diff < -180) {
      // Wrapped up, subtract 360 to continue
      unwrapped.push(unwrapped[i - 1] + (phases[i] - phases[i - 1] - 360));
    } else {
      // No wrap
      unwrapped.push(unwrapped[i - 1] + diff);
    }
  }

  return unwrapped;
}

/**
 * Calculate group delay from phase response
 * Group delay = -dφ/dω = -(dφ/df) / (2π)
 *
 * @param frequencies Frequency points in Hz
 * @param phases Phase response in degrees
 * @returns Group delay in samples (or milliseconds if scaled)
 */
export function calculateGroupDelay(
  frequencies: number[],
  phases: number[],
): number[] {
  if (frequencies.length < 2) {
    return [];
  }

  const unwrappedPhases = unwrapPhase(phases);
  const groupDelay: number[] = [];

  for (let i = 0; i < frequencies.length - 1; i++) {
    const df = frequencies[i + 1] - frequencies[i];
    const dPhase = unwrappedPhases[i + 1] - unwrappedPhases[i];

    // Convert phase difference from degrees to radians
    const dPhaseRad = (dPhase * Math.PI) / 180;

    // Group delay = -dφ/dω where ω = 2πf
    const dOmega = 2 * Math.PI * df;
    const delay = -dPhaseRad / dOmega;

    groupDelay.push(delay);
  }

  // Duplicate last value
  if (groupDelay.length > 0) {
    groupDelay.push(groupDelay[groupDelay.length - 1]);
  }

  return groupDelay;
}
