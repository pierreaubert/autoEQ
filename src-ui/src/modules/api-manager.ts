// API management and data fetching functionality

import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from '@tauri-apps/plugin-dialog';

export interface SpeakerData {
  name: string;
  versions: string[];
  measurements: { [version: string]: string[] };
}

export class APIManager {
  // API data caching
  private speakers: string[] = [];
  private selectedSpeaker: string = '';
  private selectedVersion: string = '';
  private speakerData: { [key: string]: SpeakerData } = {};

  // Autocomplete data
  private autocompleteData: string[] = [];

  constructor() {
    this.loadSpeakers();
  }

  async loadSpeakers(): Promise<void> {
    try {
      console.log('Loading speakers from API...');
      const speakers = await invoke('get_speakers') as string[];

      // Ensure we have a valid array
      if (Array.isArray(speakers)) {
        this.speakers = speakers;
        console.log('Loaded speakers:', speakers.length);

        // Update speaker dropdown
        this.updateSpeakerDropdown();

        // Load autocomplete data
        this.autocompleteData = [...speakers];
      } else {
        throw new Error('Invalid response format: expected array');
      }
    } catch (error) {
      console.error('Failed to load speakers:', error);
      // No fallback - keep empty list
      this.speakers = [];
      this.autocompleteData = [];
      console.log('No speakers available from API');
    }
  }

  async loadSpeakerVersions(speaker: string): Promise<string[]> {
    try {
      console.log('Loading versions for speaker:', speaker);

      // Try the backend call with proper parameter structure
      const result = await invoke('get_speaker_versions', {
        speaker: speaker
      });

      // Handle different response formats
      let versions: string[];
      if (Array.isArray(result)) {
        versions = result as string[];
      } else if (result && typeof result === 'object' && 'versions' in result) {
        versions = (result as any).versions;
      } else {
        throw new Error('Invalid response format from backend');
      }

      // Cache the data
      if (!this.speakerData[speaker]) {
        this.speakerData[speaker] = {
          name: speaker,
          versions: versions,
          measurements: {}
        };
      } else {
        this.speakerData[speaker].versions = versions;
      }

      console.log('Loaded versions from backend:', versions);
      return versions;
    } catch (error) {
      console.warn('Backend speaker versions not available:', error);

      // Return empty array instead of fallback data
      const emptyVersions: string[] = [];

      // Cache the empty result
      if (!this.speakerData[speaker]) {
        this.speakerData[speaker] = {
          name: speaker,
          versions: emptyVersions,
          measurements: {}
        };
      } else {
        this.speakerData[speaker].versions = emptyVersions;
      }

      console.log('No versions available for speaker:', speaker);
      return emptyVersions;
    }
  }

  async loadSpeakerMeasurements(speaker: string, version: string): Promise<string[]> {
    try {
      console.log('Loading measurements for speaker:', speaker, 'version:', version);

      // Try the backend call with proper parameter structure
      const result = await invoke('get_speaker_measurements', {
        speaker: speaker,
        version: version
      });

      // Handle different response formats
      let measurements: string[];
      if (Array.isArray(result)) {
        measurements = result as string[];
      } else if (result && typeof result === 'object' && 'measurements' in result) {
        measurements = (result as any).measurements;
      } else {
        throw new Error('Invalid response format from backend');
      }

      // Cache the data
      if (!this.speakerData[speaker]) {
        this.speakerData[speaker] = {
          name: speaker,
          versions: [],
          measurements: {}
        };
      }
      this.speakerData[speaker].measurements[version] = measurements;

      console.log('Loaded measurements from backend:', measurements);
      return measurements;
    } catch (error) {
      console.warn('Backend speaker measurements not available:', error);

      // Return empty array instead of fallback data
      const emptyMeasurements: string[] = [];

      // Cache the empty result
      if (!this.speakerData[speaker]) {
        this.speakerData[speaker] = {
          name: speaker,
          versions: [],
          measurements: {}
        };
      }
      this.speakerData[speaker].measurements[version] = emptyMeasurements;

      console.log('No measurements available for speaker:', speaker, 'version:', version);
      return emptyMeasurements;
    }
  }

  private updateSpeakerDropdown(): void {
    const speakerSelect = document.getElementById('speaker') as HTMLSelectElement;
    if (!speakerSelect) return;

    // Clear existing options
    speakerSelect.innerHTML = '<option value="">Select a speaker...</option>';

    // Add speaker options
    this.speakers.forEach(speaker => {
      const option = document.createElement('option');
      option.value = speaker;
      option.textContent = speaker;
      speakerSelect.appendChild(option);
    });

    console.log('Updated speaker dropdown with', this.speakers.length, 'speakers');
  }

  async handleSpeakerChange(speaker: string): Promise<void> {
    this.selectedSpeaker = speaker;
    this.selectedVersion = '';

    const versionSelect = document.getElementById('version') as HTMLSelectElement;
    const measurementSelect = document.getElementById('measurement') as HTMLSelectElement;

    if (!versionSelect || !measurementSelect) return;

    // Clear dependent dropdowns
    versionSelect.innerHTML = '<option value="">Select a version...</option>';
    measurementSelect.innerHTML = '<option value="">Select a measurement...</option>';

    if (!speaker) {
      // Disable dropdowns if no speaker selected
      versionSelect.disabled = true;
      measurementSelect.disabled = true;
      return;
    }

    // Set loss function to "speaker-flat" when a speaker is selected
    const lossSelect = document.getElementById('loss') as HTMLSelectElement;
    if (lossSelect) {
      lossSelect.value = 'speaker-flat';
      console.log('Set loss function to speaker-flat for speaker selection');
    }

    try {
      const versions = await this.loadSpeakerVersions(speaker);

      if (versions.length > 0) {
        versions.forEach(version => {
          const option = document.createElement('option');
          option.value = version;
          option.textContent = version;
          versionSelect.appendChild(option);
        });

        // Enable version dropdown
        versionSelect.disabled = false;

        // Automatically select the first version
        versionSelect.value = versions[0];
        this.selectedVersion = versions[0];

        // Trigger version change to load measurements
        await this.handleVersionChange(versions[0]);

        console.log('Updated version dropdown for speaker:', speaker, 'with', versions.length, 'versions. Selected:', versions[0]);
      } else {
        // No versions available, keep dropdown disabled
        versionSelect.disabled = true;
        console.log('No versions available for speaker:', speaker);
      }
    } catch (error) {
      console.error('Error loading versions for speaker:', speaker, error);
      // Keep version dropdown disabled on error
      versionSelect.disabled = true;
    }
  }

  async handleVersionChange(version: string): Promise<void> {
    this.selectedVersion = version;

    const measurementSelect = document.getElementById('measurement') as HTMLSelectElement;
    if (!measurementSelect) return;

    // Clear measurement dropdown
    measurementSelect.innerHTML = '<option value="">Select a measurement...</option>';

    if (!version || !this.selectedSpeaker) {
      // Disable measurement dropdown if no version or speaker selected
      measurementSelect.disabled = true;
      return;
    }

    try {
      const measurements = await this.loadSpeakerMeasurements(this.selectedSpeaker, version);

      if (measurements.length > 0) {
        measurements.forEach(measurement => {
          const option = document.createElement('option');
          option.value = measurement;
          option.textContent = measurement;
          measurementSelect.appendChild(option);
        });

        // Enable measurement dropdown
        measurementSelect.disabled = false;

        // Automatically select the first measurement
        measurementSelect.value = measurements[0];

        console.log('Updated measurement dropdown for version:', version, 'with', measurements.length, 'measurements. Selected:', measurements[0]);
      } else {
        // No measurements available, keep dropdown disabled
        measurementSelect.disabled = true;
        console.log('No measurements available for version:', version);
      }
    } catch (error) {
      console.error('Error loading measurements for version:', version, error);
      // Keep measurement dropdown disabled on error
      measurementSelect.disabled = true;
    }
  }

  async selectCurveFile(): Promise<string | null> {
    console.log('selectCurveFile called');
    try {
      const input = document.getElementById('curve_path') as HTMLInputElement;
      if (!input) {
        console.error('Curve path input element not found');
        return null;
      }

      console.log('Opening file dialog for curve file...');

      // Enhanced dialog options for better compatibility
      const result = await openDialog({
        multiple: false,
        directory: false,
        filters: [{
          name: 'CSV Files',
          extensions: ['csv']
        }, {
          name: 'All Files',
          extensions: ['*']
        }],
        title: 'Select Input CSV File'
      });

      console.log('Dialog result:', result);

      if (result && typeof result === 'string') {
        console.log('Setting input value to:', result);
        input.value = result;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
        this.showFileSelectionSuccess('curve-path', result);
        return result;
      } else if (result === null) {
        console.log('Dialog cancelled by user');
      } else if (Array.isArray(result) && result.length > 0) {
        // Handle array result (shouldn't happen with multiple: false, but just in case)
        const filePath = result[0];
        console.log('Setting input value to (from array):', filePath);
        input.value = filePath;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
        this.showFileSelectionSuccess('curve-path', filePath);
        return filePath;
      }

      return null;
    } catch (error) {
      console.error('Error selecting curve file:', error);
      this.showFileDialogError(error);
      // Fallback: try to trigger a native file input
      return this.fallbackFileDialog('curve-path');
    }
  }

  async selectTargetFile(): Promise<string | null> {
    console.log('selectTargetFile called');
    try {
      const input = document.getElementById('target_path') as HTMLInputElement;
      if (!input) {
        console.error('Target path input element not found');
        return null;
      }

      console.log('Opening file dialog for target file...');

      // Enhanced dialog options for better compatibility
      const result = await openDialog({
        multiple: false,
        directory: false,
        filters: [{
          name: 'CSV Files',
          extensions: ['csv']
        }, {
          name: 'All Files',
          extensions: ['*']
        }],
        title: 'Select Target CSV File (Optional)'
      });

      console.log('Dialog result:', result);

      if (result && typeof result === 'string') {
        console.log('Setting input value to:', result);
        input.value = result;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
        this.showFileSelectionSuccess('target-path', result);
        return result;
      } else if (result === null) {
        console.log('Dialog cancelled by user');
      } else if (Array.isArray(result) && result.length > 0) {
        // Handle array result (shouldn't happen with multiple: false, but just in case)
        const filePath = result[0];
        console.log('Setting input value to (from array):', filePath);
        input.value = filePath;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
        this.showFileSelectionSuccess('target-path', filePath);
        return filePath;
      }

      return null;
    } catch (error) {
      console.error('Error selecting target file:', error);
      this.showFileDialogError(error);
      // Fallback: try to trigger a native file input
      return this.fallbackFileDialog('target-path');
    }
  }

  setupAutocomplete(): void {
    const speakerInput = document.getElementById('speaker') as HTMLInputElement;
    if (!speakerInput) return;

    let autocompleteContainer: HTMLElement | null = null;

    const showAutocomplete = (suggestions: string[]) => {
      this.hideAutocomplete();

      if (suggestions.length === 0) return;

      autocompleteContainer = document.createElement('div');
      autocompleteContainer.className = 'autocomplete-suggestions';
      // Check if dark mode is active
      const isDarkMode = document.body.classList.contains('dark-mode') ||
                        document.documentElement.classList.contains('dark-mode') ||
                        window.matchMedia('(prefers-color-scheme: dark)').matches;

      autocompleteContainer.style.cssText = `
        position: absolute;
        top: 100%;
        left: 0;
        right: 0;
        background: ${isDarkMode ? '#2d3748' : 'white'};
        color: ${isDarkMode ? '#e2e8f0' : '#333'};
        border: 1px solid ${isDarkMode ? '#4a5568' : '#ccc'};
        border-top: none;
        max-height: 200px;
        overflow-y: auto;
        z-index: 10000;
        box-shadow: 0 4px 8px rgba(0,0,0,${isDarkMode ? '0.3' : '0.15'});
        border-radius: 0 0 4px 4px;
      `;

      suggestions.forEach(suggestion => {
        const item = document.createElement('div');
        item.className = 'autocomplete-item';
        item.textContent = suggestion;
        item.style.cssText = `
          padding: 8px 12px;
          cursor: pointer;
          border-bottom: 1px solid ${isDarkMode ? '#4a5568' : '#eee'};
          color: ${isDarkMode ? '#e2e8f0' : '#333'};
        `;

        item.addEventListener('mouseenter', () => {
          item.style.backgroundColor = isDarkMode ? '#4a5568' : '#f0f0f0';
        });

        item.addEventListener('mouseleave', () => {
          item.style.backgroundColor = isDarkMode ? '#2d3748' : 'white';
        });

        item.addEventListener('click', (e) => {
          e.preventDefault();
          e.stopPropagation();
          speakerInput.value = suggestion;
          this.handleSpeakerChange(suggestion);
          this.hideAutocomplete();
          speakerInput.focus();
        });

        autocompleteContainer!.appendChild(item);
      });

      // Find the correct container - look for the param-item or similar container
      let inputContainer = speakerInput.parentElement;

      // Look for a suitable container (param-item, form-group, etc.)
      if (!inputContainer || (!inputContainer.classList.contains('param-item') &&
                             !inputContainer.classList.contains('autocomplete-container') &&
                             !inputContainer.classList.contains('form-group'))) {
        inputContainer = speakerInput.closest('.param-item') ||
                        speakerInput.closest('.form-group') ||
                        speakerInput.closest('.autocomplete-container');
      }

      if (inputContainer) {
        inputContainer.style.position = 'relative';
        inputContainer.appendChild(autocompleteContainer);
        console.log('Autocomplete dropdown shown with', suggestions.length, 'suggestions in container:', inputContainer.className);
      } else {
        console.warn('Could not find suitable container, appending to document body');
        // Fallback: append to body with fixed positioning
        const rect = speakerInput.getBoundingClientRect();
        autocompleteContainer.style.position = 'fixed';
        autocompleteContainer.style.top = `${rect.bottom + window.scrollY}px`;
        autocompleteContainer.style.left = `${rect.left + window.scrollX}px`;
        autocompleteContainer.style.width = `${rect.width}px`;
        autocompleteContainer.style.zIndex = '10001'; // Higher than modal z-index
        document.body.appendChild(autocompleteContainer);
        console.log('Autocomplete dropdown positioned at body level');
      }
    };

    const hideAutocomplete = () => {
      if (autocompleteContainer) {
        autocompleteContainer.remove();
        autocompleteContainer = null;
      }
    };

    this.hideAutocomplete = hideAutocomplete;

    speakerInput.addEventListener('input', (e) => {
      const value = (e.target as HTMLInputElement).value.toLowerCase();

      if (value.length < 2) {
        hideAutocomplete();
        return;
      }

      const suggestions = this.autocompleteData
        .filter(item => item.toLowerCase().includes(value))
        .slice(0, 10);

      showAutocomplete(suggestions);
    });

    speakerInput.addEventListener('blur', () => {
      // Delay hiding to allow click events on suggestions
      setTimeout(hideAutocomplete, 150);
    });

    document.addEventListener('click', (e) => {
      if (!speakerInput.contains(e.target as Node) &&
          !autocompleteContainer?.contains(e.target as Node)) {
        hideAutocomplete();
      }
    });
  }

  private hideAutocomplete: () => void = () => {};

  async loadDemoAudioList(): Promise<string[]> {
    let audioList: string[];
    try {
      // Try to get demo audio list from backend first
      audioList = await invoke('get_demo_audio_list') as string[];
      console.log('Loaded demo audio list from backend:', audioList);
    } catch (error) {
      console.log('Backend demo audio list not available, using local files');
      // Fallback: Use actual demo audio files from public/demo-audio/
      audioList = [
        'classical.wav',
        'country.wav',
        'edm.wav',
        'female_vocal.wav',
        'jazz.wav',
        'piano.wav',
        'rock.wav'
      ];
      console.log('Using local demo audio files:', audioList);
    }

    const demoAudioSelect = document.getElementById('demo_audio_select') as HTMLSelectElement;
    if (demoAudioSelect) {
      // Clear existing options
      demoAudioSelect.innerHTML = '<option value="">Select demo audio...</option>';

      // Add a special option for loading from file
      const loadFromFileOption = document.createElement('option');
      loadFromFileOption.value = 'load_from_file';
      loadFromFileOption.textContent = 'Load from file...';
      demoAudioSelect.appendChild(loadFromFileOption);

      // Add a separator
      const separator = document.createElement('option');
      separator.disabled = true;
      separator.textContent = '──────────';
      demoAudioSelect.appendChild(separator);

      // Add audio options from the determined list
      audioList.forEach(audio => {
        const option = document.createElement('option');
        option.value = audio.replace('.wav', ''); // Remove .wav for the value
        option.textContent = audio.replace('.wav', '').replace(/_/g, ' ').replace(/\b\w/g, l => l.toUpperCase());
        demoAudioSelect.appendChild(option);
      });
    }

    return audioList;
  }

  async getDemoAudioUrl(audioName: string): Promise<string | null> {
    try {
      // Try to get URL from backend first
      const url = await invoke('get_demo_audio_url', { audio_name: audioName }) as string;
      console.log('Got demo audio URL from backend:', url);
      return url;
    } catch (error) {
      console.log('Backend demo audio URL not available, using local file path');

      // Fallback: Use local file path
      const fileName = audioName.endsWith('.wav') ? audioName : `${audioName}.wav`;
      const localUrl = `/demo-audio/${fileName}`;
      console.log('Using local demo audio URL:', localUrl);
      return localUrl;
    }
  }

  // Validation helpers
  validateOptimizationParams(formData: FormData): { isValid: boolean; errors: string[] } {
    const errors: string[] = [];

    // Debug: Log all form data
    console.log('Form validation - FormData contents:');
    for (const [key, value] of formData.entries()) {
      console.log(`  ${key}: ${value}`);
    }

    // Helper function to get and validate numeric values with defaults
    const getNumericValue = (key: string, defaultValue: number, min?: number, max?: number): number => {
      const str = formData.get(key) as string;
      if (!str || str.trim() === '') {
        console.log(`${key}: using default value ${defaultValue} (form value was empty)`);
        return defaultValue;
      }
      const value = parseFloat(str);
      if (isNaN(value)) {
        console.log(`${key}: using default value ${defaultValue} (form value "${str}" was not a number)`);
        return defaultValue;
      }
      return value;
    };

    // Use default values if form elements don't exist (using HTML form field names)
    const numFilters = getNumericValue('num_filters', 5);
    if (numFilters < 1 || numFilters > 20) {
      errors.push('Number of filters must be between 1 and 20');
    }

    const sampleRate = getNumericValue('sample_rate', 48000);
    if (sampleRate < 8000 || sampleRate > 192000) {
      errors.push('Sample rate must be between 8000 and 192000 Hz');
    }

    const maxDb = getNumericValue('max_db', 6.0);
    const minDb = getNumericValue('min_db', -1.0);
    if (maxDb <= minDb) {
      errors.push('Max dB must be greater than Min dB');
    }

    const maxQ = getNumericValue('max_q', 10);
    const minQ = getNumericValue('min_q', 0.1);
    if (maxQ <= minQ || minQ <= 0) {
      errors.push('Max Q must be greater than Min Q, and Min Q must be positive');
    }

    const maxFreq = getNumericValue('max_freq', 20000);
    const minFreq = getNumericValue('min_freq', 20);
    if (maxFreq <= minFreq || minFreq <= 0) {
      errors.push('Max frequency must be greater than Min frequency, and Min frequency must be positive');
    }

    const inputType = formData.get('input_source') as string;
    if (inputType === 'api') {
      const speaker = formData.get('speaker') as string;
      const version = formData.get('version') as string;
      const measurement = formData.get('measurement') as string;

      if (!speaker) errors.push('Speaker selection is required');
      if (!version) errors.push('Version selection is required');
      if (!measurement) errors.push('Measurement selection is required');
    } else if (inputType === 'file') {
      const curvePath = formData.get('curve_path') as string;
      const targetPath = formData.get('target_path') as string;

      if (!curvePath) errors.push('Curve file is required');
      if (!targetPath) errors.push('Target file is required');
    }

    return {
      isValid: errors.length === 0,
      errors
    };
  }

  // Getters
  getSpeakers(): string[] {
    return [...this.speakers];
  }

  getSelectedSpeaker(): string {
    return this.selectedSpeaker;
  }

  getSelectedVersion(): string {
    return this.selectedVersion;
  }

  getSpeakerData(speaker: string): SpeakerData | null {
    return this.speakerData[speaker] || null;
  }

  getAutocompleteData(): string[] {
    return [...this.autocompleteData];
  }

  private showFileSelectionSuccess(inputId: string, filePath: string): void {
    const fileName = filePath.split('/').pop() || filePath;
    const message = `Selected file: ${fileName}`;
    console.log('File selection success:', message);

    // Add visual feedback to the input
    const input = document.getElementById(inputId) as HTMLInputElement;
    if (input) {
      input.style.borderColor = '#28a745'; // Green border for success
      input.title = `Selected: ${filePath}`;
      setTimeout(() => {
        input.style.borderColor = ''; // Reset border after 2 seconds
      }, 2000);
    }
  }

  private showFileDialogError(error: any): void {
    console.error('File dialog error details:', error);
    const message = `File dialog failed: ${error?.message || error}. Using fallback file picker.`;
    console.warn(message);
    this.showTemporaryMessage(message, 'error');
  }

  private fallbackFileDialog(inputId: string): Promise<string | null> {
    return new Promise((resolve) => {
      console.log('Using fallback file dialog for:', inputId);
      const input = document.getElementById(inputId) as HTMLInputElement;
      const fileInput = document.createElement('input');
      fileInput.type = 'file';
      fileInput.accept = '.csv,text/csv';
      fileInput.style.display = 'none';

      fileInput.onchange = (event) => {
        const file = (event.target as HTMLInputElement).files?.[0];
        if (file) {
          // In fallback mode, we can only get the filename, not the full path
          // This is a browser security limitation
          input.value = file.name; // Note: This gives filename, not full path
          input.dispatchEvent(new Event('input', { bubbles: true }));
          input.dispatchEvent(new Event('change', { bubbles: true }));
          this.showFallbackWarning(inputId, file.name);
          resolve(file.name);
        } else {
          resolve(null);
        }
      };

      document.body.appendChild(fileInput);
      fileInput.click();
      document.body.removeChild(fileInput);
    });
  }

  private showFallbackWarning(inputId: string, fileName: string): void {
    const message = `Using fallback file picker. Selected: ${fileName}. Note: Full file path not available in browser mode.`;
    console.warn(`Fallback mode for ${inputId}:`, message);
    this.showTemporaryMessage(message, 'warning');

    // Add visual indication to the input
    const input = document.getElementById(inputId) as HTMLInputElement;
    if (input) {
      input.style.borderColor = '#ffc107'; // Yellow border for warning
      input.title = `Fallback mode: ${fileName} (full path not available)`;
      setTimeout(() => {
        input.style.borderColor = ''; // Reset border after 3 seconds
      }, 3000);
    }
  }

  private showTemporaryMessage(message: string, type: 'error' | 'warning' | 'success' = 'error'): void {
    // Create temporary message element
    const messageDiv = document.createElement('div');
    messageDiv.textContent = message;
    messageDiv.style.cssText = `
      position: fixed;
      top: 20px;
      right: 20px;
      max-width: 400px;
      padding: 12px 16px;
      border-radius: 6px;
      font-size: 14px;
      z-index: 10000;
      box-shadow: 0 4px 12px rgba(0,0,0,0.2);
      animation: slideIn 0.3s ease-out;
      ${type === 'error' ? 'background-color: #dc3545; color: white;' :
        type === 'warning' ? 'background-color: #ffc107; color: black;' :
        'background-color: #28a745; color: white;'}
    `;

    // Add animation keyframes if not already added
    if (!document.getElementById('temp_message_styles')) {
      const style = document.createElement('style');
      style.id = 'temp_message_styles';
      style.textContent = `
        @keyframes slideIn {
          from { transform: translateX(100%); opacity: 0; }
          to { transform: translateX(0); opacity: 1; }
        }
        @keyframes slideOut {
          from { transform: translateX(0); opacity: 1; }
          to { transform: translateX(100%); opacity: 0; }
        }
      `;
      document.head.appendChild(style);
    }

    document.body.appendChild(messageDiv);

    // Remove after 4 seconds
    setTimeout(() => {
      messageDiv.style.animation = 'slideOut 0.3s ease-in forwards';
      setTimeout(() => {
        if (messageDiv.parentNode) {
          messageDiv.parentNode.removeChild(messageDiv);
        }
      }, 300);
    }, 4000);
  }
}
