package com.efiAnalytics.plugin.ecu;

/**
 * Callback interface for receiving output channel updates.
 * 
 * This is a LibreTune open-source implementation of a documented
 * plugin API interface. Not derived from proprietary code.
 */
public interface OutputChannelClient {
    
    /**
     * Called when an output channel value is updated.
     * @param channelName The name of the channel
     * @param value The current value
     */
    void setCurrentOutputChannelValue(String channelName, double value);
}
