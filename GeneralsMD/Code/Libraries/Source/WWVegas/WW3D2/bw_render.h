#if defined(_MSC_VER)
#pragma once
#endif

#ifndef BW_RENDER_H__
#define BW_RENDER_H__

#include "always.h"
#include "vector2.h"
#include "vector3.h"
#include "vector3i.h"

class BW_Render
{
	// Internal pixel buffer used by the triangle renderer
	// The buffer is not allocated or freed by this class.
	class Buffer
	{
		unsigned char* buffer;
		int scale;
		int minv;
		int maxv;
	public:
		Buffer(unsigned char* buffer, int scale);
		~Buffer();

		void Set_H_Line(int start_x, int end_x, int y);
		void Fill(unsigned char c);
		inline int Scale() const { return scale; }
	} pixel_buffer;

	Vector2* vertices;

	void Render_Preprocessed_Triangle(Vector3& xcf,Vector3i& yci);

public:
	BW_Render(unsigned char* buffer, int scale);
	~BW_Render();

	void Fill(unsigned char c);
	void Set_Vertex_Locations(Vector2* vertices,int count); // Warning! Contents are modified!
	void Render_Triangles(const unsigned long* indices,int index_count);
	void Render_Triangle_Strip(const unsigned long* indices,int index_count);
};


#endif // BW_RENDER_H__
