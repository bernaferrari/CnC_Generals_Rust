// W3DDynamicLight.cpp
// Class to handle dynamic lights.
// Author: John Ahlquist, April 2001
#include <stdlib.h>

#include "W3DDevice/GameClient/W3DDynamicLight.h"

W3DDynamicLight::W3DDynamicLight(void):
LightClass(LightClass::POINT)
{

	m_priorEnable = false;
	m_enabled = true;

}

W3DDynamicLight::~W3DDynamicLight(void)
{
}

void W3DDynamicLight::On_Frame_Update(void)
{	
	if (!m_enabled) {
		return;
	}
	Real factor = 1.0f;
	if (m_curIncreaseFrameCount>0 && m_increaseFrameCount>0) {
		// increasing 
		m_curIncreaseFrameCount--;
		factor = (m_increaseFrameCount-m_curIncreaseFrameCount)/(Real)m_increaseFrameCount;

	}	else if (m_decayFrameCount==0) {
		factor = 1.0;  // never decays,
	}	else {
		m_curDecayFrameCount--;
		if (m_curDecayFrameCount == 0) {
			m_enabled = false;
			return;
		}
		factor = m_curDecayFrameCount/(Real)m_decayFrameCount;
	}
	if (m_decayRange) {
		this->FarAttenEnd = factor*m_targetRange;
		if (FarAttenEnd < FarAttenStart) {
			FarAttenEnd = FarAttenStart;
		}
	}
	if (m_decayColor) {
		this->Ambient = m_targetAmbient*factor;
		this->Diffuse = m_targetDiffuse*factor;
	}
}

void W3DDynamicLight::setFrameFade(UnsignedInt frameIncreaseTime, UnsignedInt decayFrameTime)
{
	m_decayFrameCount = decayFrameTime;
	m_curDecayFrameCount = decayFrameTime;
	m_curIncreaseFrameCount = frameIncreaseTime;
	m_increaseFrameCount = frameIncreaseTime;
	m_targetAmbient = Ambient;
	m_targetDiffuse = Diffuse;
	m_targetRange = FarAttenEnd;
}