#pragma once

#ifndef __W3DSHADOW_H_
#define __W3DSHADOW_H_

#include "matrix4.h"
#include "GameClient/Shadow.h"

class Drawable;	//forward reference

// ShadowManager -------------------------------------------------------------
class W3DShadowManager
{

public:
	
	W3DShadowManager( void );
	~W3DShadowManager( void );
	Bool init( void );	///<initialize resources used by manager, must have valid D3D device.
	void queueShadows(Bool state) {m_isShadowScene=state;}	///<flags system to process shadows on next render call.

	// shadow list management
	void Reset( void );
	Shadow* addShadow( RenderObjClass *robj,Shadow::ShadowTypeInfo *shadowInfo=NULL, Drawable *draw=NULL);	///< adds shadow caster to rendering system.
	void removeShadow(Shadow *shadow);	///< removed shadow from rendering system and frees its resources.
	void removeAllShadows(void); ///< Remove all shadows.
	void setShadowColor(UnsignedInt color) { m_shadowColor=color;}	///<sets the shadow color and alpha, value in ARGB format.
	UnsignedInt getShadowColor() { return m_shadowColor;}	///<gets the shadow color and alpha, value in ARGB format.
	void setLightPosition(Int lightIndex, Real x, Real y, Real z);	///<sets the position of a specific light source.
	void setTimeOfDay(TimeOfDay tod);
	void invalidateCachedLightPositions(void);	///<forces shadow volumes to update regardless of last lightposition
	Vector3 &getLightPosWorld(Int lightIndex);	///<returns the position of specified light source.
	Bool	isShadowScene(void)	{return m_isShadowScene;}
	inline void setStencilShadowMask(int mask) {m_stencilShadowMask=mask;}	///<mask used to mask out stencil bits used for storing occlusion/playerColor
	inline Int getStencilShadowMask(void)	{return m_stencilShadowMask;}

	// rendering
	void RenderShadows( void );
	void ReleaseResources(void);
	Bool ReAcquireResources(void);

protected:

		Bool	m_isShadowScene;	///<flag if current scene needs shadows.  No shadows on pre-pass and 2D.
		UnsignedInt m_shadowColor;	///<color and alpha for all shadows in scene.
		Int m_stencilShadowMask;
};  // end class W3DShadowManager

extern W3DShadowManager *TheW3DShadowManager;

#endif	//__W3DSHADOW_H_
