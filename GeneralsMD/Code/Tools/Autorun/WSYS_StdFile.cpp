//
// Project:   WSYS Library
//
// Module:    IO_
//
// File name: IO_StdFile.cpp
//
// Created:   4/23/01
//
//----------------------------------------------------------------------------

//----------------------------------------------------------------------------
//         Includes                                                      
//----------------------------------------------------------------------------

#include <stdio.h>
#include <fcntl.h>
#include <io.h>
#include <string.h>
#include <sys/stat.h>

#include "wsys_StdFile.h"
									

//----------------------------------------------------------------------------
//         Externals                                                     
//----------------------------------------------------------------------------



//----------------------------------------------------------------------------
//         Defines                                                         
//----------------------------------------------------------------------------



//----------------------------------------------------------------------------
//         Private Types                                                     
//----------------------------------------------------------------------------



//----------------------------------------------------------------------------
//         Private Data                                                     
//----------------------------------------------------------------------------



//----------------------------------------------------------------------------
//         Public Data                                                      
//----------------------------------------------------------------------------



//----------------------------------------------------------------------------
//         Private Prototypes                                               
//----------------------------------------------------------------------------



//----------------------------------------------------------------------------
//         Private Functions                                               
//----------------------------------------------------------------------------

//=================================================================
// StdFile::StdFile
//=================================================================

StdFile::StdFile()
: m_handle(-1)
{

}


//----------------------------------------------------------------------------
//         Public Functions                                                
//----------------------------------------------------------------------------


//=================================================================
// StdFile::~StdFile	
//=================================================================

StdFile::~StdFile()
{
	if( m_handle != -1 )
	{
		_close( m_handle );
		m_handle = -1;
	}

	File::close();

}

//=================================================================
// StdFile::open	
//=================================================================
/**
  *	This function opens a file using the standard C open() call. Access flags
	* are mapped to the appropriate open flags. Returns true if file was opened
	* successfully.
	*/
//=================================================================

Bool StdFile::open( const Char *filename, Int access )
{
	if( !File::open( filename, access) )
	{
		return FALSE;
	}

	/* here we translate WSYS file access to the std C equivalent */

	int flags = 0;

	if(m_access & CREATE)			flags |= _O_CREAT;
	if(m_access & TRUNCATE)		flags |= _O_TRUNC;
	if(m_access & APPEND)			flags |= _O_APPEND;
	if(m_access & TEXT)				flags |= _O_TEXT;
	if(m_access & BINARY)			flags |= _O_BINARY;

	if((m_access & READWRITE )== READWRITE )
	{
		flags |= _O_RDWR;
	}
	else if(m_access & WRITE)
	{
		flags |= _O_WRONLY;
	}
	else
		flags |= _O_RDONLY;

	m_handle = _open( filename, flags , _S_IREAD | _S_IWRITE);

	if( m_handle == -1 )
	{
		goto error;
	}

	if ( m_access & APPEND )
	{
		if ( seek ( 0, END ) < 0 )
		{
			goto error;
		}
	}

	return TRUE;

error:

	close();

	return FALSE;
}

//=================================================================
// StdFile::close 	
//=================================================================
/**
	* Closes the current file if it is open.
  * Must call StdFile::close() for each successful StdFile::open() call.
	*/
//=================================================================

void StdFile::close( void )
{
	File::close();
}

//=================================================================
// StdFile::read 
//=================================================================

Int StdFile::read( void *buffer, Int bytes )
{
	if( !m_open || !buffer )
	{
		return -1;
	}

	return _read( m_handle, buffer, bytes );
}

//=================================================================
// StdFile::write 
//=================================================================

Int StdFile::write( void *buffer, Int bytes )
{

	if( !m_open || !buffer )
	{
		return -1;
	}

	return _write( m_handle, buffer, bytes );

}

//=================================================================
// StdFile::seek 
//=================================================================

Int StdFile::seek( Int pos, seekMode mode)
{
	int lmode;

	switch( mode )
	{
		case START:
			lmode = SEEK_SET;
			break;
		case CURRENT:
			lmode = SEEK_CUR;
			break;
		case END:
			lmode = SEEK_END;
			break;
		default:
			// bad seek mode
			return -1;
	}

	return _lseek( m_handle, pos, lmode );

}

