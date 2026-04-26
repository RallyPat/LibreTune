import { useState } from 'react';
import {
  Rocket,
  FolderPlus,
  Settings as SettingsIcon,
  Plug,
  Table2,
  Bot,
  LineChart,
  Save,
  BookOpen,
  Lightbulb,
  type LucideIcon,
} from 'lucide-react';
import { Dialog, Button } from '../common';
import './QuickStartGuide.css';

interface QuickStartStep {
  id: string;
  title: string;
  description: string;
  icon: LucideIcon;
  instructions: string[];
  tips?: string[];
}

interface QuickStartGuideProps {
  isOpen: boolean;
  onClose: () => void;
}

/**
 * QuickStartGuide Component
 * 
 * Interactive step-by-step guide for new users to:
 * - Create a project
 * - Load or create a tune
 * - Connect to ECU
 * - Explore key features
 */
export default function QuickStartGuide({ isOpen, onClose }: QuickStartGuideProps) {
  const [currentStep, setCurrentStep] = useState(0);

  const steps: QuickStartStep[] = [
    {
      id: 'welcome',
      title: 'Welcome to LibreTune',
      description: 'Quick-Start Guide',
      icon: Rocket,
      instructions: [
        'This guide will walk you through the essential steps to get started.',
        'You\'ll learn how to create a project, load a tune, and connect to your ECU.',
        'Each step includes tips and explanations to help you understand LibreTune.',
      ],
    },
    {
      id: 'create-project',
      title: 'Step 1: Create a Project',
      description: 'Organize your tuning work',
      icon: FolderPlus,
      instructions: [
        '1. Click "File → New Project" (or press Ctrl+N)',
        '2. Give your project a descriptive name (e.g., "My 4-Cyl NA Speeduino")',
        '3. Select your ECU type from the list:',
        '   • Speeduino: Arduino-based open-source ECU',
        '   • rusEFI: Professional STM32-based tuning platform',
        '   • FOME: Enhanced rusEFI variant',
        '   • epicEFI: rusEFI variant for epicECU boards',
        '   • MegaSquirt: MS2/MS3 systems',
        '4. Choose a template or start from scratch',
        '5. Click "Create Project"',
      ],
      tips: [
        'Templates include pre-configured settings for common engine types',
        'Each ECU type has different menus and capabilities',
      ],
    },
    {
      id: 'load-tune',
      title: 'Step 2: Load or Create a Tune',
      description: 'Set up your ECU configuration',
      icon: SettingsIcon,
      instructions: [
        'You have three options:',
        'Option A: Start with a new blank tune',
        '   • File → New Tune (creates default values)',
        'Option B: Import an existing tune',
        '   • File → Open Tune and select .xml or .msq file',
        '   • LibreTune automatically detects and parses the format',
        'Option C: Load directly from ECU',
        '   • Connect to ECU first (see Step 3)',
        '   • Click "Tools → Load from ECU"',
      ],
      tips: [
        'Tunes are saved in MSQ format (TunerStudio compatible)',
        'You can export tunes as CSV for analysis',
      ],
    },
    {
      id: 'connect-ecu',
      title: 'Step 3: Connect to ECU',
      description: 'Establish serial communication',
      icon: Plug,
      instructions: [
        '1. Connect your ECU to your computer via USB',
        '2. Click "Connect" button or File → Connect to ECU',
        '3. Select the serial port (usually /dev/ttyACM0 or COM3+)',
        '4. Confirm the baud rate (9600-115200, varies by ECU)',
        '5. Click "Connect" to establish communication',
        '6. LibreTune will automatically sync with your ECU',
      ],
      tips: [
        'Status bar shows connection state (Connected/Disconnected)',
        'If connection fails, check driver installation and port selection',
        'Demo mode available: File → Settings → Enable Demo Mode',
      ],
    },
    {
      id: 'explore-tables',
      title: 'Step 4: Explore Fuel & Ignition Maps',
      description: 'Understanding VE and timing tables',
      icon: Table2,
      instructions: [
        '1. Click "Tables" in the sidebar',
        '2. Select a table from the menu (e.g., "Fuel → VE Table")',
        '3. The 2D table editor opens showing fuel values (VE)',
        '   • Rows: Engine RPM',
        '   • Columns: Engine Load (MAP, TPS, Airflow)',
        '   • Values: Fuel amount (percentage)',
        '4. Click on any cell to edit its value',
        '5. Use toolbar buttons:',
        '   • = (Set Equal) - Set selected cells to average',
        '   • > (Increase) - Add percentage to selected cells',
        '   • < (Decrease) - Subtract percentage',
        '   • * (Scale) - Multiply all values',
        '   • / (Interpolate) - Smooth between corners',
        '   • s (Smooth) - Weighted average filter',
      ],
      tips: [
        'Right-click for more options and advanced operations',
        'Ctrl+Z to undo, Ctrl+Y to redo changes',
        'Use 3D view for better visualization: View → 3D Table',
      ],
    },
    {
      id: 'autotune',
      title: 'Step 5: Auto-Tune Your Tables',
      description: 'Data-driven optimization',
      icon: Bot,
      instructions: [
        '1. Click "Tuning → AutoTune"',
        '2. Configure AutoTune settings:',
        '   • Select target table (usually VE table)',
        '   • Set target AFR (e.g., 14.7 for gasoline)',
        '   • Set authority limits (max change per cell)',
        '   • Configure filters (cell locks, TPS rate, etc.)',
        '3. Go for a test drive and capture data',
        '4. Review heat maps showing:',
        '   • Cell Weighting: How much data each cell got',
        '   • Cell Change: Magnitude of recommended changes',
        '5. Apply recommendations with "Send to Table"',
        '6. Iterate: Drive → Tune → Repeat for refinement',
      ],
      tips: [
        'Start with authority limits of ±2-5% per cycle',
        'Lock cells with insufficient data',
        'Multiple driving sessions improve results',
      ],
    },
    {
      id: 'dashboard',
      title: 'Step 6: Monitor with Dashboard',
      description: 'Real-time data visualization',
      icon: LineChart,
      instructions: [
        '1. Click "Dashboard" tab at the bottom',
        '2. View gauges showing live ECU data:',
        '   • RPM, AFR, Coolant temp, Intake air temp',
        '   • TPS, MAP, Battery voltage, etc.',
        '3. Customize the dashboard:',
        '   • Right-click on any gauge',
        '   • Change values, colors, or positions',
        '   • Enable Designer Mode for free movement',
        '4. Create multiple dashboards:',
        '   • Tools → New Dashboard',
        '   • Choose from templates (Basic, Racing, Tuning)',
        '5. Switch between dashboards using the dropdown',
      ],
      tips: [
        '13 different gauge types available',
        'Toggle Designer Mode to lock/unlock gauge positions',
        'Right-click background for layout options',
      ],
    },
    {
      id: 'save-burn',
      title: 'Step 7: Save & Burn to ECU',
      description: 'Persist your changes',
      icon: Save,
      instructions: [
        '1. Save your tune locally:',
        '   • File → Save Tune (or Ctrl+S)',
        '   • Choose a filename and location',
        '2. Burn to ECU:',
        '   • File → Burn to ECU (or Alt+B)',
        '   • Confirm burning to avoid accidental overwrites',
        '   • Watch progress bar during write',
        '3. Create backups:',
        '   • File → Restore Points → Create Restore Point',
        '   • Saves timestamped backup of your tune',
        '4. Version control (Git):',
        '   • File → Tune History to view all changes',
        '   • Automatic or manual commits (configurable)',
      ],
      tips: [
        'Always backup before experimenting',
        'Keep multiple restore points at different stages',
        'Use descriptive commit messages',
      ],
    },
    {
      id: 'next-steps',
      title: 'What\'s Next?',
      description: 'Continue learning',
      icon: BookOpen,
      instructions: [
        'Congratulations! You now know the core workflow.',
        'To deepen your knowledge:',
        '1. Read the comprehensive User Manual (Help → Manual)',
        '2. Explore advanced features:',
        '   • Diagnostic loggers (tooth logger, composite logger)',
        '   • Table comparison tools',
        '   • Performance calculator',
        '   • Custom keyboard shortcuts',
        '   • Light/dark themes and accessibility options',
        '3. Practice with demo mode before your first live tune',
        '4. Join the community: Discord/Forum for support',
      ],
      tips: [
        'The User Manual has detailed tutorials with screenshots',
        'Start with Demo Mode to get comfortable with the UI',
        'Consider a test vehicle or dyno run for live data',
      ],
    },
  ];

  const handleNext = () => {
    if (currentStep < steps.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      onClose();
    }
  };

  const handlePrev = () => {
    if (currentStep > 0) {
      setCurrentStep(currentStep - 1);
    }
  };

  const step = steps[currentStep];
  const isLastStep = currentStep === steps.length - 1;
  const isFirstStep = currentStep === 0;
  const progress = ((currentStep + 1) / steps.length) * 100;

  const StepIcon = step.icon;
  const titleNode = (
    <div className="qs-title-row">
      <div className="quick-start-icon"><StepIcon size={28} aria-hidden /></div>
      <div className="quick-start-title-block">
        <span className="qs-title-main">{step.title}</span>
        <span className="qs-title-sub">{step.description}</span>
      </div>
    </div>
  );

  return (
    <Dialog open={isOpen} onClose={onClose} title={titleNode} size="md" className="quick-start-dialog-wrapper">
      <div className="quick-start-progress-bar">
        <div className="progress-fill" style={{ width: `${progress}%` }} />
      </div>

      <Dialog.Body className="quick-start-content">
        <div className="instructions-section">
          <h3>Instructions</h3>
          <ol className="instructions-list">
            {step.instructions.map((instruction, idx) => (
              <li key={idx}>{instruction}</li>
            ))}
          </ol>
        </div>

        {step.tips && step.tips.length > 0 && (
          <div className="tips-section">
            <h4><Lightbulb size={14} aria-hidden /> Tips</h4>
            <ul className="tips-list">
              {step.tips.map((tip, idx) => (
                <li key={idx}>{tip}</li>
              ))}
            </ul>
          </div>
        )}
      </Dialog.Body>

      <Dialog.Footer className="quick-start-footer">
        <div className="step-indicator">
          Step {currentStep + 1} of {steps.length}
        </div>

        <div className="quick-start-controls">
          <Button variant="secondary" onClick={handlePrev} disabled={isFirstStep}>
            Back
          </Button>
          <Button variant="primary" onClick={handleNext}>
            {isLastStep ? 'Finish' : 'Next'}
          </Button>
        </div>
      </Dialog.Footer>
    </Dialog>
  );
}
