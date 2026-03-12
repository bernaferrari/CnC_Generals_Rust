#if defined(_MSC_VER)
#pragma once
#endif

#ifndef LINESEG_H
#define LINESEG_H

#include "always.h"
#include "vector3.h"
#include "castres.h"

class TriClass;
class AABoxClass;
class OBBoxClass;
class PlaneClass;
class SphereClass;
class Matrix3D;


class LineSegClass
{

public:

	LineSegClass(void) { }
	LineSegClass(const Vector3 & p0,const Vector3 & p1) : P0(p0), P1(p1) { recalculate(); }
	LineSegClass(const LineSegClass & that,const Matrix3D & tm) { Set(that,tm); }

	void					Set(const Vector3 & p0,const Vector3 & p1) { P0 = p0; P1 = p1; recalculate(); }
	void					Set(const LineSegClass & that,const Matrix3D & tm);
	void					Set_Random(const Vector3 & min,const Vector3 & max);

	const Vector3 &	Get_P0() const { return P0; }				// start point
	const Vector3 &	Get_P1() const { return P1; }				// end point
	const Vector3 &	Get_DP() const { return DP; }				// difference of the two points
	const Vector3 &	Get_Dir() const { return Dir; }			// normalized direction.
	float					Get_Length() const { return Length; }	// length of the segment

	void					Compute_Point(float t,Vector3 * set) const	{ Vector3::Add(P0,t*DP,set); }
	
	Vector3				Find_Point_Closest_To(const Vector3 &pos) const;
	bool					Find_Intersection (const LineSegClass &other_line, Vector3 *p1, float *fraction1, Vector3 *p2, float *fraction2) const;

protected:

	void					recalculate(void) { DP = P1 - P0; Dir = DP; Dir.Normalize(); Length = DP.Length(); }

	Vector3				P0;		// start point
	Vector3				P1;		// end point
	Vector3				DP;		// difference of the two points
	Vector3				Dir;		// normalized direction.
	float					Length;	// length of the segment
};


#endif
