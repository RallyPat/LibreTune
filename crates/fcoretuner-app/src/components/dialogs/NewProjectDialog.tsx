//! New Project Dialog
//!
//! Dialog for creating a new ECU tuning project.
//! Allows user to specify project name, description, and INI definition selection.

export interface NewProjectDialog {
  project_name: string;
  description: string;
  ini_path: string;
  copy_from_existing: boolean;
}

export function createNewProjectDialog(): NewProjectDialog {
  return {
    project_name: "",
    description: "",
    ini_path: "",
    copy_from_existing: false,
  };
}

export interface ProjectInfo {
  id: string;
  name: string;
  description?: string;
  ini_name: string;
  last_modified: string;
}
