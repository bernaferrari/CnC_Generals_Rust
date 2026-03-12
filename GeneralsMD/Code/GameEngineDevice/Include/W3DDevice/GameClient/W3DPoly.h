// FILE: W3DPoly.h /////////////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  W3DPoly.h
//
// Created:    Mark Wilczynski, Jan 2002
//
// Desc:       Generic Polgon operations.
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DPOLY_H_
#define __W3DPOLY_H_

#include "vector3.h"
#include "plane.h"
#include "simplevec.h"

//-------------------------------------------------------------------------------------------------
/**VisPolyClass - This class is used to clip a polygon to a plane.  Useful for manually
	* clipping polys to the frustum or other geometry.  Based on internal WW3D2 code. */
//-------------------------------------------------------------------------------------------------

class ClipPolyClass
{
public:
	void Reset(void);
	void Add_Vertex(const Vector3 & point);
	void Clip(const PlaneClass & plane,ClipPolyClass & dest) const;

	SimpleDynVecClass<Vector3> Verts;
};

#endif //__W3DPOLY_H_
