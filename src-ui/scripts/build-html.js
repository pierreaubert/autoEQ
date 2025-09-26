#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

function readTemplate(fileName) {
  const filePath = path.join(__dirname, '..', 'src', 'templates', fileName);
  if (!fs.existsSync(filePath)) {
    throw new Error(`Template file not found: ${filePath}`);
  }
  return fs.readFileSync(filePath, 'utf8');
}

function buildIndex() {
  const templates = {
    head: readTemplate('head.html'),
    dataAcquisition: readTemplate('data-acquisition.html'),
    eqDesign: readTemplate('eq-design.html'),
    optimizationFineTuning: readTemplate('optimization-fine-tuning.html'),
    formActions: readTemplate('form-actions.html'),
    rightPanel: readTemplate('right-panel.html'),
    audioControls: readTemplate('audio-controls.html'),
    optimizationModal: readTemplate('optimization-modal.html')
  };

  const indexHtml = `<!DOCTYPE html>
<html lang="en">
    ${templates.head}
    <body>
        <div class="app">
            <main class="main-content">
                <div class="left-panel" id="left_panel">
                    <form id="autoeq_form" class="parameter-form">
                        ${templates.dataAcquisition}

                        ${templates.eqDesign}

                        ${templates.optimizationFineTuning}
                    </form>

                    ${templates.formActions}
                </div>

                <div class="resizer" id="resizer"></div>

                ${templates.rightPanel}
            </main>

            ${templates.audioControls}
        </div>

        ${templates.optimizationModal}
    </body>
</html>`;

  const outputPath = path.join(__dirname, '..', 'index.html');
  fs.writeFileSync(outputPath, indexHtml, 'utf8');
  console.log('✓ Built index.html from partials');
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    buildIndex();
  } catch (error) {
    console.error('Error building HTML:', error.message);
    process.exit(1);
  }
}

export { buildIndex };
