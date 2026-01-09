package io.libretune.pluginhost;

import com.google.gson.JsonObject;

import javax.swing.*;
import java.awt.*;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Introspects Swing component trees and serializes to JSON.
 * Also handles event dispatch from React back to Swing components.
 */
public class SwingIntrospector {
    private static void log(String message) {
        System.err.println("[SwingIntrospector] " + message);
    }
    
    private static void logError(String message, Throwable e) {
        System.err.println("[SwingIntrospector] ERROR: " + message);
        if (e != null) {
            e.printStackTrace(System.err);
        }
    }
    
    // Map of component ID to component for event dispatch
    private final Map<String, Map<String, Component>> pluginComponents = new HashMap<>();
    private int componentIdCounter = 0;
    
    /**
     * Introspect a Swing component tree and return JSON representation.
     */
    public Map<String, Object> introspect(Component component) {
        String pluginId = "default";
        pluginComponents.computeIfAbsent(pluginId, k -> new HashMap<>());
        return introspectComponent(pluginId, component);
    }
    
    private Map<String, Object> introspectComponent(String pluginId, Component component) {
        Map<String, Object> node = new LinkedHashMap<>();
        
        // Generate unique ID
        String id = "c" + (++componentIdCounter);
        node.put("id", id);
        pluginComponents.get(pluginId).put(id, component);
        
        // Component type
        node.put("componentType", component.getClass().getSimpleName());
        
        // Bounds
        Map<String, Integer> bounds = new LinkedHashMap<>();
        bounds.put("x", component.getX());
        bounds.put("y", component.getY());
        bounds.put("width", component.getWidth());
        bounds.put("height", component.getHeight());
        node.put("bounds", bounds);
        
        // Properties
        Map<String, Object> props = new LinkedHashMap<>();
        props.put("enabled", component.isEnabled());
        props.put("visible", component.isVisible());
        
        if (component instanceof JComponent jc) {
            if (jc.getToolTipText() != null) {
                props.put("tooltip", jc.getToolTipText());
            }
        }
        
        // Component-specific properties
        extractComponentProperties(component, props);
        node.put("properties", props);
        
        // Layout info (if container)
        if (component instanceof Container container) {
            LayoutManager layout = container.getLayout();
            if (layout != null) {
                node.put("layout", describeLayout(layout));
            }
            
            // Children
            List<Map<String, Object>> children = new ArrayList<>();
            for (Component child : container.getComponents()) {
                Map<String, Object> childNode = introspectComponent(pluginId, child);
                
                // Add layout constraint
                Object constraint = getLayoutConstraint(container, child);
                if (constraint != null) {
                    childNode.put("layoutConstraint", constraint);
                }
                
                children.add(childNode);
            }
            
            if (!children.isEmpty()) {
                node.put("children", children);
            }
        }
        
        return node;
    }
    
    private void extractComponentProperties(Component component, Map<String, Object> props) {
        if (component instanceof JLabel label) {
            props.put("text", label.getText());
            props.put("horizontalAlignment", label.getHorizontalAlignment());
        } else if (component instanceof JButton button) {
            props.put("text", button.getText());
        } else if (component instanceof JTextField textField) {
            props.put("text", textField.getText());
            props.put("columns", textField.getColumns());
            props.put("editable", textField.isEditable());
        } else if (component instanceof JTextArea textArea) {
            props.put("text", textArea.getText());
            props.put("rows", textArea.getRows());
            props.put("columns", textArea.getColumns());
            props.put("editable", textArea.isEditable());
        } else if (component instanceof JCheckBox checkBox) {
            props.put("text", checkBox.getText());
            props.put("selected", checkBox.isSelected());
        } else if (component instanceof JRadioButton radioButton) {
            props.put("text", radioButton.getText());
            props.put("selected", radioButton.isSelected());
        } else if (component instanceof JComboBox<?> comboBox) {
            props.put("selectedIndex", comboBox.getSelectedIndex());
            List<String> items = new ArrayList<>();
            for (int i = 0; i < comboBox.getItemCount(); i++) {
                items.add(String.valueOf(comboBox.getItemAt(i)));
            }
            props.put("items", items);
        } else if (component instanceof JSlider slider) {
            props.put("value", slider.getValue());
            props.put("min", slider.getMinimum());
            props.put("max", slider.getMaximum());
            props.put("orientation", slider.getOrientation());
        } else if (component instanceof JProgressBar progressBar) {
            props.put("value", progressBar.getValue());
            props.put("min", progressBar.getMinimum());
            props.put("max", progressBar.getMaximum());
            props.put("indeterminate", progressBar.isIndeterminate());
        } else if (component instanceof JTabbedPane tabbedPane) {
            props.put("selectedIndex", tabbedPane.getSelectedIndex());
            List<String> tabTitles = new ArrayList<>();
            for (int i = 0; i < tabbedPane.getTabCount(); i++) {
                tabTitles.add(tabbedPane.getTitleAt(i));
            }
            props.put("tabTitles", tabTitles);
        } else if (component instanceof JTable table) {
            props.put("rowCount", table.getRowCount());
            props.put("columnCount", table.getColumnCount());
            
            // Column names
            List<String> columnNames = new ArrayList<>();
            for (int i = 0; i < table.getColumnCount(); i++) {
                columnNames.add(table.getColumnName(i));
            }
            props.put("columnNames", columnNames);
            
            // Table data (limit rows for performance)
            int maxRows = Math.min(table.getRowCount(), 100);
            List<List<Object>> data = new ArrayList<>();
            for (int row = 0; row < maxRows; row++) {
                List<Object> rowData = new ArrayList<>();
                for (int col = 0; col < table.getColumnCount(); col++) {
                    rowData.add(table.getValueAt(row, col));
                }
                data.add(rowData);
            }
            props.put("data", data);
        }
    }
    
    private Map<String, Object> describeLayout(LayoutManager layout) {
        Map<String, Object> info = new LinkedHashMap<>();
        
        if (layout instanceof BorderLayout bl) {
            info.put("type", "BorderLayout");
            info.put("hgap", bl.getHgap());
            info.put("vgap", bl.getVgap());
        } else if (layout instanceof GridBagLayout) {
            info.put("type", "GridBagLayout");
        } else if (layout instanceof FlowLayout fl) {
            info.put("type", "FlowLayout");
            info.put("alignment", fl.getAlignment());
            info.put("hgap", fl.getHgap());
            info.put("vgap", fl.getVgap());
        } else if (layout instanceof BoxLayout bl) {
            info.put("type", "BoxLayout");
            info.put("axis", bl.getAxis());
        } else if (layout instanceof GridLayout gl) {
            info.put("type", "GridLayout");
            info.put("rows", gl.getRows());
            info.put("cols", gl.getColumns());
            info.put("hgap", gl.getHgap());
            info.put("vgap", gl.getVgap());
        } else if (layout instanceof CardLayout cl) {
            info.put("type", "CardLayout");
            info.put("hgap", cl.getHgap());
            info.put("vgap", cl.getVgap());
        } else {
            info.put("type", layout.getClass().getSimpleName());
        }
        
        return info;
    }
    
    private Object getLayoutConstraint(Container parent, Component child) {
        LayoutManager layout = parent.getLayout();
        
        if (layout instanceof BorderLayout bl) {
            Object constraint = bl.getConstraints(child);
            return constraint != null ? constraint.toString() : null;
        } else if (layout instanceof GridBagLayout gbl) {
            GridBagConstraints gbc = gbl.getConstraints(child);
            if (gbc != null) {
                Map<String, Object> c = new LinkedHashMap<>();
                c.put("gridx", gbc.gridx);
                c.put("gridy", gbc.gridy);
                c.put("gridwidth", gbc.gridwidth);
                c.put("gridheight", gbc.gridheight);
                c.put("weightx", gbc.weightx);
                c.put("weighty", gbc.weighty);
                c.put("anchor", gbc.anchor);
                c.put("fill", gbc.fill);
                c.put("insetsTop", gbc.insets.top);
                c.put("insetsLeft", gbc.insets.left);
                c.put("insetsBottom", gbc.insets.bottom);
                c.put("insetsRight", gbc.insets.right);
                return c;
            }
        }
        
        return null;
    }
    
    /**
     * Dispatch an event from React to a Swing component.
     */
    public void dispatchEvent(String pluginId, String componentId, String eventType, JsonObject eventData) {
        Map<String, Component> components = pluginComponents.get(pluginId);
        if (components == null) {
            log("WARN: No components found for plugin: " + pluginId);
            return;
        }
        
        Component component = components.get(componentId);
        if (component == null) {
            log("WARN: Component not found: " + componentId);
            return;
        }
        
        log("Dispatching " + eventType + " to " + componentId + " (" + component.getClass().getSimpleName() + ")");
        
        // Dispatch on EDT
        SwingUtilities.invokeLater(() -> {
            try {
                dispatchEventToComponent(component, eventType, eventData);
            } catch (Exception e) {
                logError("Error dispatching event: " + e.getMessage(), e);
            }
        });
    }
    
    private void dispatchEventToComponent(Component component, String eventType, JsonObject eventData) {
        switch (eventType) {
            case "action":
                if (component instanceof AbstractButton button) {
                    button.doClick();
                }
                break;
                
            case "textChange":
                if (component instanceof JTextField textField) {
                    String text = eventData.get("text").getAsString();
                    textField.setText(text);
                } else if (component instanceof JTextArea textArea) {
                    String text = eventData.get("text").getAsString();
                    textArea.setText(text);
                }
                break;
                
            case "stateChange":
                if (component instanceof JCheckBox checkBox) {
                    boolean selected = eventData.get("selected").getAsBoolean();
                    checkBox.setSelected(selected);
                } else if (component instanceof JRadioButton radioButton) {
                    boolean selected = eventData.get("selected").getAsBoolean();
                    radioButton.setSelected(selected);
                }
                break;
                
            case "itemSelect":
                if (component instanceof JComboBox<?> comboBox) {
                    int index = eventData.get("selectedIndex").getAsInt();
                    comboBox.setSelectedIndex(index);
                }
                break;
                
            case "sliderChange":
                if (component instanceof JSlider slider) {
                    int value = eventData.get("value").getAsInt();
                    slider.setValue(value);
                }
                break;
                
            case "tableEdit":
                if (component instanceof JTable table) {
                    int row = eventData.get("row").getAsInt();
                    int col = eventData.get("column").getAsInt();
                    String value = eventData.get("value").getAsString();
                    table.setValueAt(value, row, col);
                }
                break;
                
            case "tableSelect":
                if (component instanceof JTable table) {
                    // Clear and set selection
                    table.clearSelection();
                    var selectedRows = eventData.getAsJsonArray("selectedRows");
                    for (var row : selectedRows) {
                        table.addRowSelectionInterval(row.getAsInt(), row.getAsInt());
                    }
                }
                break;
                
            default:
                log("WARN: Unknown event type: " + eventType);
        }
    }
}
