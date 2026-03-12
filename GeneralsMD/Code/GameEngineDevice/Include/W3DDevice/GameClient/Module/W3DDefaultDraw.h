// FILE: W3DModelDraw.h /////////////////////////////////////////////////////////////////////////
// Author: Colin Day, November 2001
// Desc:   Default client update module
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DDEFAULTDRAW_H_
#define __W3DDEFAULTDRAW_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/GameType.h"
#include "Common/DrawModule.h"
#include "Common/FileSystem.h"	// this is only here to pull in LOAD_TEST_ASSETS

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;
class RenderObjClass;
class FXList;

//-------------------------------------------------------------------------------------------------
/** The default client update module */
//-------------------------------------------------------------------------------------------------

//-------------------------------------------------------------------------------------------------
class W3DDefaultDraw : public DrawModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( W3DDefaultDraw, "W3DDefaultDraw" )
	MAKE_STANDARD_MODULE_MACRO( W3DDefaultDraw )

public:

	W3DDefaultDraw( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	/// the draw method
	virtual void doDrawModule(const Matrix3D* transformMtx);

	virtual void setShadowsEnabled(Bool enable);
	virtual void releaseShadows(void) {};	///< we don't care about preserving temporary shadows.	
	virtual void allocateShadows(void) {};	///< we don't care about preserving temporary shadows.
	virtual void setFullyObscuredByShroud(Bool fullyObscured);
	virtual void reactToTransformChange(const Matrix3D* oldMtx, const Coord3D* oldPos, Real oldAngle);
	virtual void reactToGeometryChange() { }

private:

#ifdef LOAD_TEST_ASSETS
	RenderObjClass*		m_renderObject;										///< W3D Render object for this drawable
	Shadow*				m_shadow;													///< Updates/Renders shadows of this object
#endif
};

#endif // __W3DDEFAULTDRAW_H_

