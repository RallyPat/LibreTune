package com.efiAnalytics.plugin.ecu;

import java.awt.Dimension;
import java.util.ArrayList;

/**
 * TunerStudio Plugin API - ControllerParameter class.
 * Represents a tunable parameter (constant) in the ECU.
 */
public class ControllerParameter {
    
    public static final String PARAM_CLASS_BITS = "bits";
    public static final String PARAM_CLASS_SCALAR = "scalar";
    public static final String PARAM_CLASS_ARRAY = "array";
    
    private String paramClass = PARAM_CLASS_SCALAR;
    private int decimalPlaces = 2;
    private Dimension shape;
    private String units = "";
    private double min = 0;
    private double max = 100;
    private ArrayList<String> optionDescriptions;
    private double[][] arrayValues;
    private double scalarValue;
    private String stringValue;
    
    public ControllerParameter() {}
    
    public String getParamClass() {
        return paramClass;
    }
    
    public void setParamClass(String paramClass) {
        this.paramClass = paramClass;
    }
    
    public int getDecimalPlaces() {
        return decimalPlaces;
    }
    
    public void setDecimalPlaces(int decimalPlaces) {
        this.decimalPlaces = decimalPlaces;
    }
    
    public Dimension getShape() {
        return shape;
    }
    
    public void setShape(Dimension shape) {
        this.shape = shape;
    }
    
    public String getUnits() {
        return units;
    }
    
    public void setUnits(String units) {
        this.units = units;
    }
    
    public double getMin() {
        return min;
    }
    
    public void setMin(double min) {
        this.min = min;
    }
    
    public double getMax() {
        return max;
    }
    
    public void setMax(double max) {
        this.max = max;
    }
    
    public ArrayList<String> getOptionDescriptions() {
        return optionDescriptions;
    }
    
    public void setOptionDescriptions(ArrayList<String> optionDescriptions) {
        this.optionDescriptions = optionDescriptions;
    }
    
    public double[][] getArrayValues() {
        return arrayValues;
    }
    
    public void setArrayValues(double[][] arrayValues) {
        this.arrayValues = arrayValues;
    }
    
    public double getScalarValue() {
        return scalarValue;
    }
    
    public void setScalarValue(double scalarValue) {
        this.scalarValue = scalarValue;
    }
    
    public String getStringValue() {
        return stringValue;
    }
    
    public void setStringValue(String stringValue) {
        this.stringValue = stringValue;
    }
}
