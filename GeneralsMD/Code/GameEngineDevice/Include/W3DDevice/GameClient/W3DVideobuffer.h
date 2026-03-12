//
// Project:    Generals
//
// File name:  W3DDevice/GameClient/W3DVideoBuffer.h
//
// Created:    10/23/01
//
//----------------------------------------------------------------------------

#pragma once

#ifndef __W3DDEVICE_GAMECLIENT_W3DVIDEOBUFFER_H_
#define __W3DDEVICE_GAMECLIENT_W3DVIDEOBUFFER_H_


//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

#include "GameClient/VideoPlayer.h"

//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------

class TextureClass;
class SurfaceClass;

//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------

//===============================
// W3DVideoBuffer
//===============================
/**
  * Video buffer interface class to a W3D TextureClass
	*/
//===============================


class W3DVideoBuffer : public VideoBuffer
{
	protected:

		TextureClass	*m_texture;
		SurfaceClass	*m_surface;

	public:

		W3DVideoBuffer( Type format );
		virtual ~W3DVideoBuffer();

		virtual	Bool		allocate( UnsignedInt width, UnsignedInt height); ///< Allocates buffer
		virtual void		free( void);					///< Free buffer
		virtual	void*		lock( void );					///< Returns memory pointer to start of buffer
		virtual void		unlock( void );				///< Release buffer
		virtual Bool		valid( void );				///< Is the buffer valid to use

		TextureClass		*texture( void );			///< Returns texture object

		static WW3DFormat TypeToW3DFormat( VideoBuffer::Type format );
		static VideoBuffer::Type W3DFormatToType( WW3DFormat w3dFormat );
};


//----------------------------------------------------------------------------
//           Inlining                                                       
//----------------------------------------------------------------------------

inline TextureClass* W3DVideoBuffer::texture( void ) { return m_texture; }

#endif // __W3DDEVICE_GAMECLIENT_W3DVIDEOBUFFER_H_
