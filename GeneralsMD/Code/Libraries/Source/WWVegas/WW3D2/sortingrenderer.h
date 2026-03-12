#if defined(_MSC_VER)
#pragma once
#endif

#ifndef SORTING_RENDERER_H
#define SORTING_RENDERER_H

#include "always.h"

class SortingNodeStruct;
class SphereClass;

class SortingRendererClass
{
	static bool _EnableTriangleDraw;

	static void Flush_Sorting_Pool();
	static void Insert_To_Sorting_Pool(SortingNodeStruct* state);

public:
	static void Insert_Triangles(
		const SphereClass& bounding_sphere,
		unsigned short start_index, 
		unsigned short polygon_count,
		unsigned short min_vertex_index,
		unsigned short vertex_count);

	static void Insert_Triangles(
		unsigned short start_index, 
		unsigned short polygon_count,
		unsigned short min_vertex_index,
		unsigned short vertex_count);

	static void Insert_VolumeParticle(
		const SphereClass& bounding_sphere,
		unsigned short start_index, 
		unsigned short polygon_count,
		unsigned short min_vertex_index,
		unsigned short vertex_count,
		unsigned short layerCount);

	static void Flush();
	static void Deinit();
	
	static void SetMinVertexBufferSize( unsigned val );

	static void _Enable_Triangle_Draw(bool enable) { _EnableTriangleDraw=enable; }
	static bool _Is_Triangle_Draw_Enabled() { return _EnableTriangleDraw; }
};

#endif

