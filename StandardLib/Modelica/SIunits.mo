package SIunits
    type Real = Real;
    type Time = Real(quantity="Time", unit="s");
    type Position = Real(quantity="Length", unit="m");
    type Velocity = Real(quantity="Velocity", unit="m/s");
    type Acceleration = Real(quantity="Acceleration", unit="m/s2");
    type Mass = Real(quantity="Mass", unit="kg", min=0.0);
    type Force = Real(quantity="Force", unit="N");
    type Pressure = Real(quantity="Pressure", unit="Pa");
    type Density = Real(quantity="Density", unit="kg/m3", min=0.0);
    type Temperature = Real(quantity="ThermodynamicTemperature", unit="K", min=0.0);
    type Angle = Real(quantity="Angle", unit="rad");
    type AngularVelocity = Real(quantity="AngularVelocity", unit="rad/s");
    type Voltage = Real(quantity="ElectricPotential", unit="V");
    type Current = Real(quantity="ElectricCurrent", unit="A");
    type Resistance = Real(quantity="Resistance", unit="Ohm");
    type Power = Real(quantity="Power", unit="W");
    type Energy = Real(quantity="Energy", unit="J");
end SIunits;
