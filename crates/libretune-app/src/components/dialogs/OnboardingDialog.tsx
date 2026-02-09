import { useState } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import './OnboardingDialog.css';

interface OnboardingStep {
  id: string;
  title: string;
  description: string;
  icon: string;
  details: string[];
  action?: {
    label: string;
    handler: () => void | Promise<void>;
  };
}

interface OnboardingDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onComplete: () => void;
}

/**
 * OnboardingDialog Component
 * 
 * Comprehensive first-run welcome experience with:
 * - Welcome intro
 * - Feature overview (6 key features)
 * - Quick-start guide
 * - Links to resources
 */
export default function OnboardingDialog({ isOpen, onClose, onComplete }: OnboardingDialogProps) {
  const [currentStep, setCurrentStep] = useState(0);

  const steps: OnboardingStep[] = [
    {
      id: 'welcome',
      title: 'Welcome to LibreTune',
      description: 'Professional ECU Tuning Software',
      icon: '🚗',
      details: [
        'LibreTune is an open-source ECU tuning platform supporting Speeduino, rusEFI, FOME, epicEFI, and MegaSquirt.',
        'Built with modern technology: Rust backend + React frontend + Tauri desktop framework.',
        'Fully keyboard navigable with accessibility features for all users.',
      ],
    },
    {
      id: 'projects',
      title: 'Create Your First Project',
      description: 'Organize your tuning work',
      icon: '📁',
      details: [
        '1. Click "File → New Project" or use the welcome screen',
        '2. Select your ECU type (Speeduino, rusEFI, etc.)',
        '3. Choose from built-in templates or start from scratch',
        '4. Connect to your ECU via serial port',
        '5. Load or create a tune file (MSQ format)',
      ],
    },
    {
      id: 'tables',
      title: 'Edit Fuel & Ignition Maps',
      description: 'Professional 2D/3D table editing',
      icon: '📊',
      details: [
        '2D Editor: Click "Tables" to view and edit fuel, ignition, and auxiliary tables',
        'Toolbar: Use =, >, <, *, /, s for Set Equal, Increase, Decrease, Scale, Interpolate, Smooth',
        '3D View: Visualize table values in 3D space with live cursor tracking',
        'History Trail: Follow your optimization in real-time',
        'Copy/Paste: Transfer values between cells using Ctrl+C/Ctrl+V',
      ],
    },
    {
      id: 'autotune',
      title: 'Auto-Tune with AI Assistance',
      description: 'Data-driven fuel table optimization',
      icon: '🤖',
      details: [
        '1. Click "Tuning → AutoTune" to start optimization',
        '2. Capture live data from your ECU during driving',
        '3. Review heat maps showing cell weighting and recommended changes',
        '4. Apply recommendations with authority limits',
        '5. Iteratively refine your tune for target AFR',
        'Features: Cell locking, filter settings, authority limits, undo/redo',
      ],
    },
    {
      id: 'dashboard',
      title: 'Real-Time Monitoring',
      description: 'Professional dashboard with gauges',
      icon: '📈',
      details: [
        'Dashboard shows 13 gauge types: analog dials, bars, sweep gauges, line graphs, etc.',
        'Right-click to customize: change gauges, colors, positions',
        'Designer Mode: Lock/unlock gauges, grid snap, free movement',
        'Multiple Dashboards: Basic, Racing, Tuning layouts pre-configured',
        'Themes: Auto-apply different themes with one click',
      ],
    },
    {
      id: 'keyboard',
      title: 'Keyboard Shortcuts',
      description: 'Power-user workflow',
      icon: '⌨️',
      details: [
        'Customizable hotkeys: Settings → Keyboard Shortcuts',
        'Table editor: Arrow keys navigate, =,>,<,*,/,s operate',
        'Dialog navigation: Tab/Shift+Tab move between fields, Escape closes',
        'Conflicts detected automatically',
        'Export/import binding schemes for team sharing',
      ],
    },
    {
      id: 'resources',
      title: 'Helpful Resources',
      description: 'Learn more about LibreTune',
      icon: '📚',
      details: [
        '📖 User Manual: Click "Help → Manual" for comprehensive guides',
        '🔧 Settings: File → Settings to configure preferences, units, hotkeys',
        '💾 Git Integration: Auto-save your work with version control (File → Tune History)',
        '🌐 Online Repos: Automatic INI and tune file downloads from GitHub',
        '💬 Community: Join Discord for support and discussions',
      ],
      action: {
        label: 'Open User Manual',
        handler: async () => {
          try {
            await openUrl('https://libretune.dev/manual/');
          } catch (e) {
            console.error('Failed to open manual:', e);
          }
        },
      },
    },
  ];

  const handleNext = () => {
    if (currentStep < steps.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      handleComplete();
    }
  };

  const handlePrev = () => {
    if (currentStep > 0) {
      setCurrentStep(currentStep - 1);
    }
  };

  const handleComplete = () => {
    onComplete();
    onClose();
  };

  const step = steps[currentStep];
  const isLastStep = currentStep === steps.length - 1;
  const isFirstStep = currentStep === 0;

  if (!isOpen) return null;

  return (
    <div className="onboarding-overlay">
      <div className="onboarding-dialog">
        <div className="onboarding-header">
          <div className="onboarding-icon">{step.icon}</div>
          <div>
            <h2>{step.title}</h2>
            <p>{step.description}</p>
          </div>
          <button className="onboarding-close" onClick={onClose} aria-label="Close onboarding">
            ×
          </button>
        </div>

        <div className="onboarding-content">
          <ul>
            {step.details.map((detail, idx) => (
              <li key={idx}>{detail}</li>
            ))}
          </ul>
        </div>

        {step.action && (
          <div className="onboarding-action">
            <button 
              className="onboarding-action-btn"
              onClick={step.action.handler}
            >
              {step.action.label}
            </button>
          </div>
        )}

        <div className="onboarding-footer">
          <div className="onboarding-progress">
            {steps.map((_, idx) => (
              <div
                key={idx}
                className={`progress-dot ${idx === currentStep ? 'active' : ''} ${idx < currentStep ? 'completed' : ''}`}
                onClick={() => setCurrentStep(idx)}
                role="button"
                tabIndex={0}
                aria-label={`Go to step ${idx + 1}: ${steps[idx].title}`}
              />
            ))}
          </div>

          <div className="onboarding-controls">
            <button
              onClick={handlePrev}
              disabled={isFirstStep}
              className="onboarding-btn onboarding-btn-secondary"
            >
              ← Previous
            </button>
            <button
              onClick={handleNext}
              className="onboarding-btn onboarding-btn-primary"
            >
              {isLastStep ? 'Get Started' : 'Next →'}
            </button>
          </div>
        </div>

        <label className="onboarding-checkbox">
          <input
            type="checkbox"
            defaultChecked={false}
            onChange={(e) => {
              // User can re-enable onboarding later in settings
              if (!e.target.checked) {
                localStorage.setItem('libretune-onboarding-completed', 'false');
              }
            }}
          />
          Show this welcome on next startup
        </label>
      </div>
    </div>
  );
}
