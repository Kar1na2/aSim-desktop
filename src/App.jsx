import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [message, setMessage] = useState("");
  const [isError, setIsError] = useState(false); // Tracks if the message is an error
  const [isLogin, setIsLogin] = useState(true);

  const handleSubmit = async (e) => {
    e.preventDefault();
    setMessage("");
    setIsError(false); // Reset state on new submission
    
    try {
      const command = isLogin ? 'login' : 'register';
      const response = await invoke(command, { 
        username: username, 
        password: password 
      });
      
      setMessage(response);
      
    } catch (error) {
      setIsError(true);
      
      // Extract the string from Tauri's serialized Rust enum { "Client": "..." }
      let errorMsg = "An unknown error occurred";
      if (typeof error === 'string') {
        errorMsg = error;
      } else if (error && error.Client) {
        errorMsg = error.Client;
      } else if (error && error.Internal) {
        errorMsg = error.Internal;
      }
      
      setMessage(errorMsg); 
    }
  };

  const toggleMode = () => {
    setIsLogin(!isLogin);
    setMessage(""); 
    setIsError(false);
  };

  return (
    <div className="mobile-view">
      <h1>{isLogin ? "Welcome Back" : "Create Account"}</h1>
      
      <form onSubmit={handleSubmit} className="auth-form">
        <input 
          type="text" 
          placeholder="Username" 
          value={username}
          onChange={(e) => setUsername(e.target.value)} 
          required
        />
        <input 
          type="password" 
          placeholder="Password" 
          value={password}
          onChange={(e) => setPassword(e.target.value)} 
          required
        />
        <button type="submit">
          {isLogin ? "Log In" : "Sign Up"}
        </button>
      </form>

      {/* Conditionally apply the error or success CSS class */}
      {message && (
        <p className={`status-message ${isError ? 'error-text' : 'success-text'}`}>
          {message}
        </p>
      )}

      <div className="toggle-container">
        <p>{isLogin ? "Don't have an account?" : "Already have an account?"}</p>
        <button className="toggle-btn" type="button" onClick={toggleMode}>
          {isLogin ? "Register here" : "Authenticate here"}
        </button>
      </div>
    </div>
  );
}

export default App;