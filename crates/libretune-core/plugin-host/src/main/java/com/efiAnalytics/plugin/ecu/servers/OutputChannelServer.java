package com.efiAnalytics.plugin.ecu.servers;

import com.efiAnalytics.plugin.ecu.ControllerException;
import com.efiAnalytics.plugin.ecu.OutputChannel;
import com.efiAnalytics.plugin.ecu.OutputChannelClient;

/**
 * Interface for accessing realtime output channel data.
 * 
 * This is a LibreTune open-source implementation of a documented
 * plugin API interface. Not derived from proprietary code.
 */
public interface OutputChannelServer {
    
    /**
     * Subscribe to an output channel.
     * @param ecuConfigName The ECU configuration
     * @param outputChannelName The channel name
     * @param client The callback client
     * @throws ControllerException if subscription fails
     */
    void subscribe(String ecuConfigName, String outputChannelName, OutputChannelClient client) 
        throws ControllerException;
    
    /**
     * Unsubscribe all channels for an ECU configuration.
     * @param ecuConfigName The ECU configuration
     */
    void unsubscribeConfiguration(String ecuConfigName);
    
    /**
     * Unsubscribe a specific client.
     * @param client The callback client
     */
    void unsubscribe(OutputChannelClient client);
    
    /**
     * Get all available output channel names.
     * @param ecuConfigName The ECU configuration
     * @return Array of channel names
     * @throws ControllerException if retrieval fails
     */
    String[] getOutputChannels(String ecuConfigName) throws ControllerException;
    
    /**
     * Get an output channel by name.
     * @param ecuConfigName The ECU configuration
     * @param outputChannelName The channel name
     * @return The output channel
     * @throws ControllerException if channel not found
     */
    OutputChannel getOutputChannel(String ecuConfigName, String outputChannelName) 
        throws ControllerException;
}
