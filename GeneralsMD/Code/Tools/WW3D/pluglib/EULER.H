/* $Header: /Commando/Code/Tools/max2w3d/EULER.H 3     10/28/97 6:08p Greg_h $ */
/*********************************************************************************************** 
 ***                            Confidential - Westwood Studios                              *** 
 *********************************************************************************************** 
 *                                                                                             * 
 *                 Project Name : Commando / G 3D Engine                                       * 
 *                                                                                             * 
 *                     $Archive:: /Commando/Code/Tools/max2w3d/EULER.H                        $* 
 *                                                                                             * 
 *                      $Author:: Greg_h                                                      $* 
 *                                                                                             * 
 *                     $Modtime:: 10/14/97 3:08p                                              $* 
 *                                                                                             * 
 *                    $Revision:: 3                                                           $* 
 *                                                                                             * 
 *---------------------------------------------------------------------------------------------* 
 * Functions:                                                                                  * 
 * - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - */


#ifndef EULER_H
#define EULER_H

#include <Max.h>


/*********************************************************************

	Euler Order Types 

	When creating an EulerAngles object, use one of the below
	constants to describe the axis convention.

	XYZ - order of the axes
	s/r - whether the rotations are applied to the static or 
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
	convert matrices into a set of euler angles.

	This implementation is based on the code in Graphics Gems IV
	by Ken Shoemake.  The original article is on page 222.
	
*********************************************************************/
class EulerAnglesClass
{

public:

	EulerAnglesClass(void) : Order(0) { Angle[0] = 0.0; Angle[1] = 0.0; Angle[2] = 0.0; };
	EulerAnglesClass(const Matrix3 & from,int order);
	void		From_Matrix(const Matrix3 & from,int order);
	void		To_Matrix(Matrix3 & M);

	double	Get_Angle(int i);

private:

	double	Angle[3];
	int		Order;

};


#endif /*EULER_H*/