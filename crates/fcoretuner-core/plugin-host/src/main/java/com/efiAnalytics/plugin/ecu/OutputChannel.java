package com.efiAnalytics.plugin.ecu;

/**
 * TunerStudio Plugin API - OutputChannel class.
 * Represents a realtime output channel from the ECU.
 */
public class OutputChannel {
    
    private String name;
    private String units;
    private double minValue;
    private double maxValue;
    private String formula;
    
    public OutputChannel() {}
    
    public String getName() {
        return name;
    }
    
    public void setName(String name) {
        this.name = name;
    }
    
    public String getUnits() {
        return units;
    }
    
    public void setUnits(String units) {
        this.units = units;
    }
    
    public double getMinValue() {
        return minValue;
    }
    
    public void setMinValue(double minValue) {
        this.minValue = minValue;
    }
    
    public double getMaxValue() {
        return maxValue;
    }
    
    public void setMaxValue(double maxValue) {
        this.maxValue = maxValue;
    }
    
    public String getFormula() {
        return formula;
    }
    
    public void setFormula(String formula) {
        this.formula = formula;
    }
}
