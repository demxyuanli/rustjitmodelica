model FluidTank
  // Parameters
  parameter Real L(start=1.0);      // Length
  parameter Real rho(start=1000.0); // Density (Water)
  parameter Real cp(start=4200.0);  // Specific Heat Capacity
  parameter Real Q_in(start=5000.0);// Heating Power (5kW)
  parameter Real h(start=10.0);     // Heat Transfer Coefficient
  parameter Real T_amb(start=20.0); // Ambient Temperature

  // Algebraic Variables (Intermediate)
  Real Volume(start=1.0);
  Real Area(start=6.0);
  Real Mass(start=1000.0);

  // State Variables
  Real T(start=20.0); // Temperature

equation
  // Geometric equations (Cube)
  Volume = L * L * L;
  Area = 6.0 * L * L;
  
  // Physical properties
  Mass = rho * Volume;

  // Energy conservation: m * cp * der(T) = Q_in - Q_loss
  der(T) = (Q_in - h * Area * (T - T_amb)) / (Mass * cp);
end FluidTank;
