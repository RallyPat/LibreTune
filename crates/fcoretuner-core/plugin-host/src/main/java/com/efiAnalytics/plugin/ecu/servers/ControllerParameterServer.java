package com.efiAnalytics.plugin.ecu.servers;

import com.efiAnalytics.plugin.ecu.ControllerException;
import com.efiAnalytics.plugin.ecu.ControllerParameter;
import com.efiAnalytics.plugin.ecu.ControllerParameterChangeListener;

/**
 * TunerStudio Plugin API - ControllerParameterServer interface.
 * Provides access to ECU parameters (constants).
 */
public interface ControllerParameterServer {
    
    /**
     * Subscribe to parameter changes.
     * @param ecuConfigName The ECU configuration
     * @param parameterName The parameter name
     * @param listener The callback listener
     */
    void subscribe(String ecuConfigName, String parameterName, ControllerParameterChangeListener listener);
    
    /**
     * Unsubscribe from all parameters.
     * @param listener The callback listener
     */
    void unsubscribe(ControllerParameterChangeListener listener);
    
    /**
     * Get all parameter names.
     * @param ecuConfigName The ECU configuration
     * @return Array of parameter names
     */
    String[] getParameterNames(String ecuConfigName);
    
    /**
     * Get a parameter by name.
     * @param ecuConfigName The ECU configuration
     * @param parameterName The parameter name
     * @return The parameter
     */
    ControllerParameter getControllerParameter(String ecuConfigName, String parameterName);
    
    /**
     * Update a scalar parameter.
     * @param ecuConfigName The ECU configuration
     * @param parameterName The parameter name
     * @param value The new value
     */
    void updateParameter(String ecuConfigName, String parameterName, double value);
    
    /**
     * Update an array parameter.
     * @param ecuConfigName The ECU configuration
     * @param parameterName The parameter name
     * @param values The new values
     */
    void updateParameter(String ecuConfigName, String parameterName, double[][] values);
    
    /**
     * Update a bits/string parameter.
     * @param ecuConfigName The ECU configuration
     * @param parameterName The parameter name
     * @param value The new value
     */
    void updateParameter(String ecuConfigName, String parameterName, String value);
    
    /**
     * Burn data to ECU.
     * @param ecuConfigName The ECU configuration
     */
    void burnData(String ecuConfigName) throws ControllerException;
}
