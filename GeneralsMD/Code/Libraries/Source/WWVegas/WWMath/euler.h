#if defined(_MSC_VER)
#pragma once
#endif

#ifndef EULER_H
#define EULER_H

#include "always.h"
#include "matrix3d.h"
#include "quat.h"


/*********************************************************************

	Euler Order Types 

	When creating an EulerAngles object, use one of the below
	constants to describe the axis convention.

	XYZ - order of the axes
	s/r - whether the rotations are applied to a static or 
	      rotating frame.

*********************************************************************/


/* static axes */
extern int	EulerOrderXYZs;
extern int	EulerOrderXYXs;
extern int	EulerOrderXZYs;
extern int	EulerOrderXZXs;
extern int	EulerOrderYZXs;
extern int	EulerOrderYZYs;
extern int	EulerOrderYXZs;
extern int	EulerOrderYXYs;
extern int	EulerOrderZXYs;
extern int	EulerOrderZXZs;
extern int	EulerOrderZYXs;
extern int	EulerOrderZYZs;

/* rotating axes */
extern int	EulerOrderXYZr;
extern int	EulerOrderXYXr;
extern int	EulerOrderXZYr;
extern int	EulerOrderXZXr;
extern int	EulerOrderYZXr;
extern int	EulerOrderYZYr;
extern int	EulerOrderYXZr;
extern int	EulerOrderYXYr;
extern int	EulerOrderZXYr;
extern int	EulerOrderZXZr;
extern int	EulerOrderZYXr;
extern int	EulerOrderZYZr;


/*********************************************************************
	
	EulerAnglesClass

	The purpose for this class is mainly for conversion.  You can
	choose a convention for the order of your rotations and then
	convert matrices into a set of euler angles.  You don't really
	want to use this at run-time to convert matrices into angles.

	The guts of this implementation is based on the article in Graphics 
	Gems IV by Ken Shoemake.  The original article is on page 222.
	
*********************************************************************/
class EulerAnglesClass
{

public:

	EulerAnglesClass(void) : Order(0) { Angle[0] = 0.0; Angle[1] = 0.0; Angle[2] = 0.0; };
	EulerAnglesClass(const Matrix3D & from,int order);
	void		From_Matrix(const Matrix3D & from,int order);
	void		To_Matrix(Matrix3D & M);

	double	Get_Angle(int i);

private:

	double	Angle[3];
	int		Order;

};


#endif /*EULER_H*/
