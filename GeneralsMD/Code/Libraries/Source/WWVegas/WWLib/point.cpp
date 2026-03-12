#ifdef NEVER


#include	"always.h"
#include	"point.h"
#include	"rect.h"


/*********************************************************************************************** 
 * Point2D::Bias_To -- Bias a point into a rectangle.                                          * 
 *                                                                                             * 
 *    It is often necessary to take a point that is relative to a rectangle and derive a       * 
 *    point that is no longer relative to the rectangle coordiates, yet still have it refer    * 
 *    to the same location.                                                                    * 
 *                                                                                             * 
 * INPUT:   rect  -- The rectangle to bias this point against.                                 * 
 *                                                                                             * 
 * OUTPUT:  Returns with a point in the rectangles coordinate space but still referring to the * 
 *          same location.                                                                     * 
 *                                                                                             * 
 * WARNINGS:   none                                                                            * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   05/26/1997 JLB : Created.                                                                 * 
 *=============================================================================================*/
Point2D const Point2D::Bias_To(Rect const & rect) const
{
	return(Point2D(X + rect.X, Y + rect.Y));
}

#endif
