// FILE: W3DPropDraw.h //////////////////////////////////////////////////////////////////////////
// Author: John Ahlquist June 2003
// Desc:   Simple prop drawing draw method.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DPropDraw_H_
#define __W3DPropDraw_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/DrawModule.h"
#include "WW3D2/Line3D.h"

//-------------------------------------------------------------------------------------------------
class W3DPropDrawModuleData : public ModuleData
{
public:
	AsciiString m_modelName;

	W3DPropDrawModuleData();
	~W3DPropDrawModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);
	// ugh, hack
	virtual const W3DPropDrawModuleData* getAsW3DPropDrawModuleData() const { return this; }
};

//-------------------------------------------------------------------------------------------------
/** W3D prop draw */
//-------------------------------------------------------------------------------------------------
class W3DPropDraw : public DrawModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( W3DPropDraw, "W3DPropDraw" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( W3DPropDraw, W3DPropDrawModuleData )

public:

	W3DPropDraw( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void doDrawModule(const Matrix3D* transformMtx);
	virtual void setShadowsEnabled(Bool enable) { }
	virtual void releaseShadows(void) {};	///< we don't care about preserving temporary shadows.	
	virtual void allocateShadows(void) {};	///< we don't care about preserving temporary shadows.
	virtual void setFullyObscuredByShroud(Bool fullyObscured) { }
	virtual void reactToTransformChange(const Matrix3D* oldMtx, const Coord3D* oldPos, Real oldAngle);
	virtual void reactToGeometryChange() { }

protected:
	Bool m_propAdded;

};

#endif // __W3DPropDraw_H_

