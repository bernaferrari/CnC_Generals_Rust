//
// Filename:     mathutil.h
// Project:      wwutil
// Author:       Tom Spencer-Smith
// Date:         June 1998
// Description:  static
//
//-----------------------------------------------------------------------------
#if defined(_MSV_VER)
#pragma once
#endif

#ifndef MATHUTIL_H
#define MATHUTIL_H

class cMathUtil
{
	public:
		static void		Angle_To_Vector(double angle, double & dx, double & dy);
      static void		Vector_To_Angle(double dx, double dy, double & angle);
		static double	Simple_Distance(double x1, double y1, double x2, double y2);
      static int		Round(double arg);
      static void		Rotate_Vector(double & vx, double & vy, double angle);

      //
      // Random numbers generated according to simple 
      // Probability Density Functions
      //
      static double  Get_Uniform_Pdf_Double(double lower, double upper);
      static double  Get_Normalized_Uniform_Pdf_Double();
      static int     Get_Uniform_Pdf_Int(int lower, int upper);
      static double  Get_Hat_Pdf_Double(double lower, double upper);
      static double  Get_Normalized_Hat_Pdf_Double();
      static int     Get_Hat_Pdf_Int(int lower, int upper);

		static const double PI_1;
		static const double PI_2;
};

#endif // MATHUTIL_H