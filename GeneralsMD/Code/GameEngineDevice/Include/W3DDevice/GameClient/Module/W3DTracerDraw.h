// FILE: W3DTracerDraw.h //////////////////////////////////////////////////////////////////////////
// Author: 
// Desc:   
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DTRACERDRAW_H_
#define __W3DTRACERDRAW_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/DrawModule.h"
#include "WW3D2/Line3D.h"

//-------------------------------------------------------------------------------------------------
/** W3D tracer draw */
//-------------------------------------------------------------------------------------------------
class W3DTracerDraw : public DrawModule, public TracerDrawInterface
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( W3DTracerDraw, "W3DTracerDraw" )
	MAKE_STANDARD_MODULE_MACRO( W3DTracerDraw )

public:

	W3DTracerDraw( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void doDrawModule(const Matrix3D* transformMtx);
	virtual void setShadowsEnabled(Bool enable) { }
	virtual void releaseShadows(void) {};	///< we don't care about preserving temporary shadows.	
	virtual void allocateShadows(void) {};	///< we don't care about preserving temporary shadows.
	virtual void setFullyObscuredByShroud(Bool fullyObscured) { }
	virtual void reactToTransformChange(const Matrix3D* oldMtx, const Coord3D* oldPos, Real oldAngle);
	virtual void reactToGeometryChange() { }

	virtual void setTracerParms(Real speed, Real length, Real width, const RGBColor& color, Real initialOpacity);

	virtual TracerDrawInterface* getTracerDrawInterface() { return this; }
	virtual const TracerDrawInterface* getTracerDrawInterface() const { return this; }

protected:

	Line3DClass *m_theTracer;			///< the tracer render object in the W3D scene
	Real m_length;								///< length of tracer
	Real m_width;									///< width of tracer
	RGBColor m_color;							///< color of tracer
	Real m_speedInDistPerFrame;		///< speed of tracer (in dist/frame)
	Real m_opacity;								///< opacity of the tracer

};

#endif // __W3DTRACERDRAW_H_

