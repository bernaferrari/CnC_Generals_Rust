#if 0

#if defined(_MSC_VER)
#pragma once
#endif

#ifndef MESHDAM_H
#define MESHDAM_H

#include "always.h"
#include "vector3.h"
#include "bittype.h"
#include "w3derr.h"

class MeshModelClass;
class ChunkLoadClass;

struct RGBStruct 
{
	uint8	R,G,B;
};

struct DamageVertexStruct
{
	int		VertexIdx;		// index of the vertex to "damage"
	Vector3	Vertex0;			// original vertex position
	Vector3	Vertex1;			// damaged vertex position
};

struct DamageColorStruct
{
	int			VertexIdx;	// index of the vertex to damage.
	RGBStruct	Color0;		// original color.
	RGBStruct	Color1;		// damaged color.
};


/*
** DamageClass - this class encapsulates the information needed
** to apply damage to meshes.  It contains replacement vertices,
** vertex colors and materials
*/
class DamageClass
{
public:

	DamageClass(void);
	~DamageClass(void);

	WW3DErrorType				Load_W3D(ChunkLoadClass & cload,MeshModelClass * basemesh);

protected:

	WW3DErrorType				read_vertices(ChunkLoadClass & cload,MeshModelClass * basemesh);
	WW3DErrorType				read_colors(ChunkLoadClass & cload,MeshModelClass * basemesh);

	int							DamageIndex;
	int							NumMaterials;
	int							NumVerts;
	int							NumColors;

	DamageVertexStruct *		Verts;
	DamageColorStruct *		Colors;	

	friend class MeshClass;
};


#endif


#endif //0