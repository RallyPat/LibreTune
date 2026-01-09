package com.efiAnalytics.plugin.ecu;

/**
 * Callback interface for receiving parameter change notifications.
 * 
 * This is a LibreTune open-source implementation of a documented
 * plugin API interface. Not derived from proprietary code.
 */
public interface ControllerParameterChangeListener {
    
    /**
     * Called when a parameter value changes.
     * @param parameterName The name of the parameter
     * @param parameter The updated parameter object
     */
    void controllerParameterChanged(String parameterName, ControllerParameter parameter);
}
