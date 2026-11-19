import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { resolveQuery } from '../utils/routing';

export default function SearchWindow() {
  const [query, setQuery] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // Focus the input automatically with preventScroll to stop any vertical shift
    if (inputRef.current) {
      inputRef.current.focus({ preventScroll: true });
    }

    const unlistenReset = listen('reset_search', () => {
      setQuery('');
      setTimeout(() => {
        if (inputRef.current) {
          inputRef.current.focus({ preventScroll: true });
          inputRef.current.select();
        }
      }, 20);
    });

    const unlistenClose = listen('close-overlay-window', () => {
      invoke('dismiss').catch(console.error);
    });

    return () => {
      unlistenReset.then((unlisten) => unlisten());
      unlistenClose.then((unlisten) => unlisten());
    };
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      const targetUrl = resolveQuery(query);
      invoke('launch_overlay', { targetUrl }).catch((err) => {
        console.error('Failed to launch overlay:', err);
      });
    } else if (e.key === 'Escape') {
      invoke('dismiss').catch(console.error);
    }
  };

  return (
    <div style={{
      width: '100%',
      height: '100%',
      margin: 0,
      padding: 0,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      backgroundColor: 'transparent',
      overflow: 'hidden'
    }}>
      <div style={{
        display: 'flex',
        alignItems: 'center',
        width: '560px',
        height: '50px',
        maxHeight: '50px',
        backgroundColor: '#202124',
        borderRadius: '25px',
        border: '1px solid #5f6368',
        boxShadow: '0 8px 24px rgba(0,0,0,0.6)',
        padding: '0 16px',
        boxSizing: 'border-box',
        overflow: 'hidden'
      }}>
        {/* Google "G" Icon */}
        <svg style={{ width: '22px', height: '22px', marginRight: '12px', flexShrink: 0 }} viewBox="0 0 24 24">
          <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/>
          <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
          <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/>
          <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/>
        </svg>

        {/* Input Field */}
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Search Google or type a URL"
          style={{
            flex: 1,
            width: '100%',
            height: '100%',
            maxHeight: '100%',
            margin: 0,
            padding: 0,
            background: 'transparent',
            border: 'none',
            outline: 'none',
            color: '#e8eaed',
            fontSize: '16px',
            fontFamily: 'Arial, sans-serif',
            overflow: 'hidden'
          }}
        />
      </div>
    </div>
  );
}

export { SearchWindow };
