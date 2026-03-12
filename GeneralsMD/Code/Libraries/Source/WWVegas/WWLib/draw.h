#ifndef DRAW_H
#define DRAW_H

#include	"convert.h"
#include	"point.h"
#include	"shapeset.h"

void Draw_Shape(Surface & surface, ConvertClass & convert, ShapeSet const * shapefile, int shapenum, Point2D const & point, Rect const & window, ShapeFlags_Type flags = SHAPE_NORMAL, unsigned char const * remap = NULL);
void Blit_Block(Surface & surface, ConvertClass & convert, Surface const & source, Rect const & sourcerect, Point2D const & point, Rect const & clipwindow, unsigned char const * remap = NULL, Blitter const * blitter = NULL);

#endif