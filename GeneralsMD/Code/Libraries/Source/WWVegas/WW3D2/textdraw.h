#if defined(_MSC_VER)
#pragma once
#endif

#ifndef TEXTDRAW_H
#define TEXTDRAW_H

#include "always.h"
#include "dynamesh.h"

// sgc : wwlib and wwmath contain different rect.h files...
#include "..\wwmath\rect.h"

class	Font3DInstanceClass;

/******************************************************************
**
** TextDrawClass
**
** This class provides a simple method to draw 2D text into a scene.
** Both strings and individual characters can be drawn to any normalized 
** screen coordinates ( 0.. 1 ), or any scale/offset. 
**  This class uses a dynamic mesh for all polygon and vertex management
**
*******************************************************************/

class TextDrawClass : public DynamicMeshClass
{

public:
	/*
	** Constructor and Destructor
	*/
	TextDrawClass( int max_chars );
	~TextDrawClass();

	// Set Coordinate Range
	void	Set_Coordinate_Ranges(	const Vector2 & param_ul, const Vector2 & param_lr, 
											const Vector2 & dest_ul, const Vector2 & dest_lr );

	// Reset all polys and verts
	virtual	void Reset( void );

	/*
	** class id of this render object
	*/
	virtual int	Class_ID(void) const	{ return CLASSID_TEXTDRAW; }

	/*
	**
	*/
	float	Get_Width( Font3DInstanceClass *font, const char *message );
	float	Get_Char_Width( Font3DInstanceClass *font, const char c );
	float	Get_Inter_Char_Width( Font3DInstanceClass *font );
	float	Get_Height( Font3DInstanceClass *font, const char *message = "" );

	/*
	** Print the given char/string with the given font at the given loation in screen pixels
	** returns the pixel width of the drawn data.
	*/
	float	Print( Font3DInstanceClass *font, char ch, float screen_x, float screen_y);
	float	Print( Font3DInstanceClass *font, const char *message, float screen_x, float screen_y);

	void	Set_Text_Color( const Vector3 & color )		{ Set_Vertex_Color(color); }

	/*
	** dump the font image (debuging)
	*/
	void	Show_Font( Font3DInstanceClass *font, float screen_x, float screen_y );

	void Quad( float x0, float y0, float x1, float y1, float u0 = 0, float v0 = 0, float u1 = 1, float v1 = 1);
	void Quad( const RectClass	& rect, const RectClass	& uv = RectClass( 0, 0, 1, 1 ) );
	void Line( const Vector2 & a, const Vector2 & b, float width );
	void Line_Ends( const Vector2 & a, const Vector2 & b, float width, float end_percent );

private:
	Vector3					TextColor;
	VertexMaterialClass	*DefaultVertexMaterial;
	ShaderClass				DefaultShader;
	Vector2					TranslateScale;
	Vector2					TranslateOffset;
	Vector2					PixelSize;
};

#endif	// TEXTDRAW_H
