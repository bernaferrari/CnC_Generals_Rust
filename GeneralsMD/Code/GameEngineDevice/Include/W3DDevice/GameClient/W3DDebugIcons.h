#pragma once

#ifndef __W3D_DEBUG_ICONS_H_
#define __W3D_DEBUG_ICONS_H_

#include "always.h"
#include "rendobj.h"
#include "vertmaterial.h"
#include "Lib/BaseType.h"

#if defined _DEBUG || defined _INTERNAL
struct DebugIcon;
//
/// W3DDebugIcons: Draws huge numbers of debug icons for pathfinding quickly.
//
//
class W3DDebugIcons : public RenderObjClass
{	

public:

	W3DDebugIcons(void);
	W3DDebugIcons(const W3DDebugIcons & src);
	W3DDebugIcons & operator = (const W3DDebugIcons &);
	~W3DDebugIcons(void);

	/////////////////////////////////////////////////////////////////////////////
	// Render Object Interface 
	/////////////////////////////////////////////////////////////////////////////
	virtual RenderObjClass *	Clone(void) const;
	virtual int						Class_ID(void) const;
	virtual void					Render(RenderInfoClass & rinfo);

	virtual bool					Cast_Ray(RayCollisionTestClass & raytest);

	virtual void					Get_Obj_Space_Bounding_Sphere(SphereClass & sphere) const;
  virtual void					Get_Obj_Space_Bounding_Box(AABoxClass & aabox) const;

protected:
	VertexMaterialClass	  	*m_vertexMaterialClass;

protected: 
	static DebugIcon				*m_debugIcons;
	static Int							m_numDebugIcons;

protected:
	enum {MAX_ICONS = 100000};
	void allocateIconsArray(void);
	void compressIconsArray(void);

public:
	static void addIcon(const Coord3D *pos, Real width, Int numFramesDuration, RGBColor color);
};
#endif // _DEBUG or _INTERNAL

#endif  // end __W3D_DEBUG_ICONS_H_
