#if defined(_MSC_VER)
#pragma once
#endif

#ifndef AAPLANE_H
#define AAPLANE_H

#include "always.h"
#include "vector3.h"


/*
** This class is used to describe an "axis-aligned" plane.  I.e, the normal
** of the plane is one of the three coordinate axes.
*/
class AAPlaneClass
{
public:

	enum AxisEnum { XNORMAL = 0, YNORMAL = 1, ZNORMAL = 2 };

	AAPlaneClass(void)																		{ }
	AAPlaneClass(AxisEnum normal,float dist) : Normal(normal),Dist(dist)		{ }

	void Set(AxisEnum normal,float dist);
	void Get_Normal(Vector3 * normal) const;

public:

	AxisEnum			Normal;
	float				Dist;

};

inline void AAPlaneClass::Set(AxisEnum normal,float dist)
{
	Normal = normal;
	Dist = dist;
}

inline void AAPlaneClass::Get_Normal(Vector3 * normal) const
{
	normal->Set(0,0,0);
	(*normal)[Normal] = 1.0f;
}

#endif
