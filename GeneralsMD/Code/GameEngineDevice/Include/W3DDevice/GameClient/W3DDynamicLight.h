// W3DDynamicLight.h
// Class to generate texture for terrain.
// Author: John Ahlquist, April 2001

#pragma once

#ifndef W3D_DYNAMIC_LIGHT_H
#define W3D_DYNAMIC_LIGHT_H

#include "WW3D2/Light.h"
#include "lib/baseType.h"
class HeightMapRenderObjClass;

/*************************************************************************
**                             W3DDynamicLight
***************************************************************************/
class W3DDynamicLight : public LightClass
{
friend class BaseHeightMapRenderObjClass;
friend class HeightMapRenderObjClass;
protected:
	/// Values used by HeightMapRenderObjClass to update the height map.
	Bool		m_priorEnable;
	Bool		m_processMe;
	

	Int			m_prevMinX, m_prevMinY, m_prevMaxX, m_prevMaxY;
	Int			m_minX, m_minY, m_maxX, m_maxY;

	Bool		m_enabled;

	Bool		m_decayRange;
	Bool		m_decayColor;
	UnsignedInt m_curDecayFrameCount;
	UnsignedInt m_curIncreaseFrameCount;
	UnsignedInt m_decayFrameCount;
	UnsignedInt m_increaseFrameCount;
	Real		m_targetRange;
	Vector3 m_targetAmbient;
	Vector3 m_targetDiffuse;


public:
	W3DDynamicLight();
	~W3DDynamicLight(void);

public:
	virtual void					On_Frame_Update(void); 

	void setEnabled(Bool enabled) { m_enabled = enabled; m_decayRange = false; m_decayFrameCount = 0; m_decayColor = false; m_increaseFrameCount = 0;};
	Bool isEnabled(void) {return m_enabled;};


	/// 0 frameIncreaseTime means it starts out full size/intensity, 0 decay time means it lasts forever.
	void setFrameFade(UnsignedInt frameIncreaseTime, UnsignedInt decayFrameTime);
	void setDecayRange(void) {m_decayRange = true;};
	void setDecayColor(void) {m_decayColor = true;};
	// Cull returns true if the terrain vertex at x,y is outside of the light's influence.
	Bool cull(Int x, Int y ) {return (x<m_minX||y<m_minY||x>m_maxX||y>m_maxY);}
};



#endif //TEXTURE_H
