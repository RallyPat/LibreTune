package com.efiAnalytics.plugin.ecu;

import com.efiAnalytics.plugin.ecu.servers.ControllerParameterServer;
import com.efiAnalytics.plugin.ecu.servers.OutputChannelServer;
import com.efiAnalytics.plugin.ecu.servers.UiSettingServer;

/**
 * Provides access to ECU controller data for plugins.
 * 
 * This is a LibreTune open-source implementation of a documented
 * plugin API class. Not derived from proprietary code.
 */
public abstract class ControllerAccess {
    
    private static ControllerAccess instance;
    
    /**
     * Protected constructor - use getInstance().
     */
    protected ControllerAccess() {
    }
    
    /**
     * Get the singleton instance.
     */
    public static ControllerAccess getInstance() {
        return instance;
    }
    
    /**
     * Initialize with server implementations.
     */
    public static void initialize(OutputChannelServer outputServer, 
                                   ControllerParameterServer parameterServer) {
        // This is called by the host to set up the servers
    }
    
    protected static void setInstance(ControllerAccess access) {
        instance = access;
    }
    
    /**
     * Get the output channel server for realtime data.
     */
    public abstract OutputChannelServer getOutputChannelServer();
    
    /**
     * Get the controller parameter server for constants.
     */
    public abstract ControllerParameterServer getControllerParameterServer();
    
    /**
     * Get the UI settings/component server for plugin preferences.
     * Note: Some plugins call this "UiComponentServer" but it returns UiSettingServer.
     * @param ecuConfigName The ECU configuration name
     * @return The UI settings server
     */
    public abstract UiSettingServer getUiComponentServer(String ecuConfigName);
    
    /**
     * Get all ECU configuration names.
     */
    public abstract String[] getEcuConfigurationNames();
    
    /**
     * Evaluate an expression.
     * @param ecuConfigName The ECU configuration
     * @param expression The expression to evaluate
     * @return The result value
     * @throws MathException if expression evaluation fails
     */
    public abstract double evaluateExpression(String ecuConfigName, String expression) 
        throws MathException;
    
    /**
     * Send a burn command to save changes to ECU.
     * @throws ControllerException if burn fails
     */
    public abstract void sendBurnCommand(String ecuConfigName) throws ControllerException;
    
    /**
     * Check if ECU is online/connected.
     */
    public boolean isOnline() {
        return true; // Default implementation
    }
    
    /**
     * Go online (connect to ECU).
     */
    public void goOnline() {
        // Default no-op
    }
    
    /**
     * Go offline (disconnect from ECU).
     */
    public void goOffline() {
        // Default no-op
    }
}
