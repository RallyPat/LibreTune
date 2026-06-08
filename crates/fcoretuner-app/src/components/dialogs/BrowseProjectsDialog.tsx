//! Browse Projects Dialog
//!
//! Dialog for browsing and opening existing ECU tuning projects.

export interface ProjectInfo {
  id: string;
  name: string;
  description?: string;
  ini_name: string;
  last_modified: string;
}

export function createProjectInfo(id: string, name: string, ini_name: string): ProjectInfo {
  return {
    id,
    name,
    description: undefined,
    ini_name,
    last_modified: "",
  };
}

export interface BrowseProjectsDialog {
  projects: ProjectInfo[];
  selected_project_id?: string;
  search_query: string;
  show_hidden_projects: boolean;
}
