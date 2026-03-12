// FILE: W3DTreeDraw.h //////////////////////////////////////////////////////////////////////////
// Author: 
// Desc:   
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DTreeDraw_H_
#define __W3DTreeDraw_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/DrawModule.h"
#include "WW3D2/Line3D.h"

//-------------------------------------------------------------------------------------------------
class W3DTreeDrawModuleData : public ModuleData
{
public:
	AsciiString m_modelName;
	AsciiString m_textureName;
	// Push aside parameters. [5/29/2003]
	UnsignedInt	m_framesToMoveOutward;
	UnsignedInt	m_framesToMoveInward;
	Real				m_maxOutwardMovement;
	Real				m_darkening;

	// Topple parameters. [7/7/2003]
	const FXList* m_toppleFX;
	const FXList* m_bounceFX;
	AsciiString m_stumpName;
	Real m_initialVelocityPercent;
	Real m_initialAccelPercent;
	Real m_bounceVelocityPercent;
  Real m_minimumToppleSpeed;
	Bool m_killWhenToppled;
	Bool m_doTopple;
	UnsignedInt m_sinkFrames; // How long it takes to sink after toppling. [7/11/2003]
	Real m_sinkDistance;		// How far it sinks.

	Bool m_doShadow;

	W3DTreeDrawModuleData();
	~W3DTreeDrawModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);
	// ugh, hack
	virtual const W3DTreeDrawModuleData* getAsW3DTreeDrawModuleData() const { return this; }
};

//-------------------------------------------------------------------------------------------------
/** W3D tree draw */
//-------------------------------------------------------------------------------------------------
class W3DTreeDraw : public DrawModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( W3DTreeDraw, "W3DTreeDraw" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( W3DTreeDraw, W3DTreeDrawModuleData )

public:

	W3DTreeDraw( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void doDrawModule(const Matrix3D* transformMtx);
	virtual void setShadowsEnabled(Bool enable) { }
	virtual void releaseShadows(void) {};	///< we don't care about preserving temporary shadows.	
	virtual void allocateShadows(void) {};	///< we don't care about preserving temporary shadows.
	virtual void setFullyObscuredByShroud(Bool fullyObscured) { }
	virtual void reactToTransformChange(const Matrix3D* oldMtx, const Coord3D* oldPos, Real oldAngle);
	virtual void reactToGeometryChange() { }

protected:
	Bool m_treeAdded;

};

#endif // __W3DTreeDraw_H_

